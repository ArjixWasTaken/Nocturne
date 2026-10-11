//! `NocturneEqualizerPage` (playing/equalizer_page.blp): six bands bound to `eq-band-*`.
use crate::config::settings;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/jeffser/Nocturne/playing/equalizer_page.ui")]
    pub struct EqualizerPage {
        #[template_child]
        pub container: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EqualizerPage {
        const NAME: &'static str = "NocturneEqualizerPage";
        type Type = super::EqualizerPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for EqualizerPage {
        fn constructed(&self) {
            self.parent_constructed();
            let s = settings();
            for i in 0..crate::player::EQ_BANDS {
                let band = gtk::Scale::new(gtk::Orientation::Vertical, Some(&gtk::Adjustment::new(0.0, -12.0, 12.0, 0.1, 1.0, 0.0)));
                band.set_inverted(true);
                band.set_draw_value(true);
                s.bind(&format!("eq-band-{i}"), &band.adjustment(), "value").build();
                for v in [6.0, 0.0, -6.0] {
                    let markup = (i == 0).then(|| format!("{v}dB"));
                    band.add_mark(v, gtk::PositionType::Top, markup.as_deref());
                }
                self.container.append(&band);
            }
        }
    }
    impl WidgetImpl for EqualizerPage {}
    impl BinImpl for EqualizerPage {}
}

glib::wrapper! {
    pub struct EqualizerPage(ObjectSubclass<imp::EqualizerPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Apply a named equalizer preset to the `eq-band-*` settings.
pub fn apply_preset(name: &str) {
    let preset: [f64; 6] = match name {
        "clarity" => [-2.0, -4.0, 2.0, 5.0, 3.0, 1.0],
        "jazz" => [4.0, 2.0, 1.0, -2.0, -5.0, -8.0],
        "rock" => [5.0, 2.0, -2.0, 3.0, 2.0, 0.0],
        "classic" => [2.0, -2.0, 0.0, 1.0, 2.0, 4.0],
        "acoustic" => [-2.0, 3.0, 1.0, -2.0, 4.0, 2.0],
        _ => [0.0; 6],
    };
    let s = settings();
    for (i, v) in preset.iter().enumerate() {
        let _ = s.set_double(&format!("eq-band-{i}"), *v);
    }
}
