use gi_ui::Drawable;
use gi_ui::components::circle::Circle;
use gi_ui::size::SizePolicy;
use gi_ui_app::Application;

fn create_ui() -> impl Drawable {
    let circle = Circle::new()
        .with_radius(SizePolicy::FillParent)
        .set_foreground_color(0xff_ff0000);

    circle
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = create_ui();
    let mut application = Application::new(200, 200)?;

    application.attach_main_drawable(Box::new(ui));

    application.run()
}
