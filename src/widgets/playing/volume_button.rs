//! `NocturneVolumeButton` (playing/volume_button.blp).
use crate::config::settings;
use adw::{prelude::*, subclass::prelude::*};
use gtk::{CompositeTemplate, glib};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arjix/Nocturne/playing/volume_button.ui")]
    pub struct VolumeButton {
        #[template_child]
        pub volume_adjustment_el: TemplateChild<gtk::Adjustment>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VolumeButton {
        const NAME: &'static str = "NocturneVolumeButton";
        type Type = super::VolumeButton;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VolumeButton {
        fn constructed(&self) {
            self.parent_constructed();
            settings().bind("volume", &*self.volume_adjustment_el, "value").build();
        }
    }
    impl WidgetImpl for VolumeButton {}
    impl BinImpl for VolumeButton {}
}

glib::wrapper! {
    pub struct VolumeButton(ObjectSubclass<imp::VolumeButton>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl VolumeButton {
    #[template_callback(function)]
    fn handle_icon_name_bind(value: f64) -> String {
        match value {
            v if v == 0.0 => "speaker-0-symbolic",
            v if v < 0.33 => "speaker-1-symbolic",
            v if v < 0.66 => "speaker-2-symbolic",
            _ => "speaker-3-symbolic",
        }
        .into()
    }

    #[template_callback]
    fn full_volume(&self, _: &gtk::Button) {
        let _ = settings().set_double("volume", 1.0);
    }

    #[template_callback]
    fn mute_volume(&self, _: &gtk::Button) {
        let _ = settings().set_double("volume", 0.0);
    }
}
