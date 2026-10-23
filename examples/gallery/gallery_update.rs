impl Gallery {
    pub fn update(&mut self, msg: Msg, _cx: &mut UpdateCtx<Msg>) {
        match msg {
            Msg::SetPage(p) => {
                self.page = p;
                self.scroll = 0.0;
            }
            Msg::SetTheme(t) => self.theme = t,
            Msg::Invoke(_) => {}
            Msg::Scroll(dy) => self.scroll = (self.scroll + dy).max(0.0),
            Msg::Edit(i, e) => {
                let _ = self.texts[i].accept(e);
            }
            Msg::Conflict(_, _) => {}
            Msg::ToggleCb(i) => self.cb[i] = !self.cb[i],
            Msg::SetRadio(g, k) => self.radio[g] = k,
            Msg::ToggleSw(i) => self.sw[i] = !self.sw[i],
            Msg::SelectOpen(id, open) => {
                if id == "select.default.open" {
                    self.select_open = open;
                } else if id == "settings-nav.theme" {
                    self.settings_theme_open = open;
                }
            }
            Msg::SelectPick(id, key) => {
                if id == "select.default.open" {
                    self.env = key;
                    self.select_open = false;
                } else if id == "settings-nav.theme" {
                    self.settings_theme = key;
                    self.settings_theme_open = false;
                }
            }
            Msg::MenuOpen(open) => self.menu_open = open,
            Msg::MenuAction(_) => self.menu_open = false,
            Msg::MenuCheck(key, value) => {
                match key {
                    "toolbar" => self.menu_toolbar = value,
                    "word-wrap" => self.menu_wrap = value,
                    _ => {}
                }
                self.menu_open = false;
            }
            Msg::MenuRadio(key) => {
                self.menu_panel = key;
                self.menu_open = false;
            }
            Msg::SetTab(line, key) => {
                if line {
                    self.tabs_line = key;
                } else {
                    self.tabs_default = key;
                }
            }
            Msg::SetNav(key) => self.nav_selected = key,
        }
    }
}
