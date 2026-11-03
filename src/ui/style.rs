use gtk4 as gtk;

const CSS: &str = r#"
window.pf { background: #0f1115; color: #e6e8ec; }
.pf headerbar { background: #14171d; box-shadow: none; border-bottom: 1px solid #232832; }
.pf .card-pf { background: #171a21; border: 1px solid #232832; border-radius: 14px; padding: 14px 16px; }
.pf .section { font-size: 11px; font-weight: 700; letter-spacing: 1px; color: #7c8594; }
.pf .freq { font-size: 11px; color: #8a93a3; }
.pf .db { font-size: 12px; font-weight: 600; font-feature-settings: "tnum"; color: #3ddbb8; min-width: 52px; }
.pf .db.zero { color: #7c8594; }
.pf .db.preamp { color: #f5b83d; }
.pf scale.fader trough { min-width: 6px; min-height: 6px; background: #262b35; border-radius: 3px; }
.pf scale.fader highlight { background: #3ddbb8; border-radius: 3px; }
.pf scale.fader.preamp highlight { background: #f5b83d; }
.pf scale.fader slider { min-width: 22px; min-height: 22px; border-radius: 11px; background: #e6e8ec; border: 2px solid #0f1115; box-shadow: 0 1px 4px rgba(0,0,0,.6); }
.pf scale.fader:focus-visible slider, .pf scale.fader slider:hover { background: #ffffff; }
.pf scale.fader.balance highlight { background: transparent; }
.pf .status-ok { color: #3ddbb8; }
.pf .status-warn { color: #f5b83d; }
.pf .status-bad { color: #ff6b6b; }
.pf .dirty { color: #f5b83d; font-weight: 600; }
.pf .version { font-size: 11px; color: #5d6574; }
.pf .sidebar-pf { background: #14171d; border-right: 1px solid #232832; padding: 14px; }
.pf .title-pf { font-size: 18px; font-weight: 700; }
.pf list.profiles { background: transparent; }
.pf list.profiles row { border-radius: 9px; padding: 8px 10px; margin-bottom: 4px; border-left: 3px solid transparent; }
.pf list.profiles row:selected { background: #222834; border-left-color: #3ddbb8; }
.pf list.profiles row:hover:not(:selected) { background: #1a1e27; }
.pf button.pill-pf { border-radius: 999px; padding: 4px 14px; }
"#;

pub fn install() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
