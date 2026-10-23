use super::super::*;
use super::brush::*;
use super::guards::{box_geometry, brush_color, resolve_render};
use super::icons::*;
use super::shadow::*;
use super::*;

impl Renderer {
    pub(super) fn draw_frame<S, M, U, V>(&mut self, be: &Backend<S, M, U, V>) -> UiResult
    where
        M: 'static,
        U: Fn(&mut S, M, &mut UpdateCtx<'_, M>),
        V: Fn(&S, &mut crate::Ui<'_, '_, M>),
    {
        self.hwnd = be.hwnd;
        // resolved theme darkness — the app-selected mode, not raw OS state
        let dark = be.rt.theme.dark;
        let forced = be.rt.forced_resolver();
        let dpi = self.dpi;
        // COM add-ref as the interface we paint through — ends the &mut
        // self borrow so peer draws can read renderer state
        let target: ID2D1RenderTarget = self
            .ensure_target()?
            .cast()
            .map_err(|e| UiError::Platform(format!("target cast: {e}")))?;
        let _tgen = self.target_gen.get();
        unsafe {
            let d = super::super::space::dpi_of(dpi) as f32;
            target.SetDpi(d, d);
        }
        // forced colors route through the SAME resolver as every other
        // role consumer — the window clear is not an exempt path
        let bg = resolve_render(
            crate::style::Color::Role(crate::theme::ColorRole::Background),
            dark,
            forced,
        );
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&bg));
        }
        let order = be.order.clone();
        for id in order {
            let Some(r) = be.rects.get(&id).copied() else {
                continue;
            };
            let Some(n) = be.rt.arena.get(id) else {
                continue;
            };
            // scroll-viewport clip — uniform for EVERY painted kind.
            // A guard so a `?` inside the draw arm can never strand the
            // pushed clip on the shared target.
            let _clip_guard = be.clips.get(&id).map(|c| {
                unsafe {
                    target.PushAxisAlignedClip(
                        &D2D_RECT_F {
                            left: c.x,
                            top: c.y,
                            right: c.x + c.width,
                            bottom: c.y + c.height,
                        },
                        D2D1_ANTIALIAS_MODE_ALIASED,
                    );
                }
                ClipGuard(&target)
            });
            let clip = D2D_RECT_F {
                left: r.x,
                top: r.y,
                right: r.x + r.width,
                bottom: r.y + r.height,
            };
            match &n.data {
                NodeData::Label {
                    text,
                    color_role,
                    wrap,
                    patch,
                    chrome,
                } => include!("frame_label.rs"),
                NodeData::Icon { icon, size, color } => include!("frame_icon.rs"),
                NodeData::Button {
                    text,
                    variant,
                    style,
                    disabled,
                    size,
                    icon_start,
                    icon_end,
                    icon_only,
                    invalid,
                    selected,
                    icon_color,
                    expanded: _,
                    motion: _,
                    tooltip: _,
                    accessible_name: _,
                } => include!("frame_button.rs"),
                NodeData::Toggle {
                    kind,
                    size,
                    checked,
                    disabled,
                    invalid,
                    label,
                    ..
                } => include!("frame_toggle.rs"),
                NodeData::Editor {
                    patch,
                    disabled,
                    invalid,
                    snapshot,
                    placeholder,
                    ..
                } => include!("frame_editor.rs"),
                NodeData::Custom { render, .. } => include!("frame_custom.rs"),
                NodeData::Container { kind, props, .. } => include!("containers_container.rs"),
                NodeData::Action {
                    style, disabled, ..
                } => include!("containers_action.rs"),
                NodeData::Popup { modal, props, .. } => include!("containers_popup.rs"),
                _ => {}
            }
        }
        match unsafe { target.EndDraw(None, None) } {
            Err(e) if e.code() == D2DERR_RECREATE_TARGET => {
                Err(UiError::Platform("D2DERR_RECREATE_TARGET".into()))
            }
            Err(e) => Err(UiError::Platform(format!("EndDraw: {e}"))),
            Ok(()) => Ok(()),
        }
    }
}
