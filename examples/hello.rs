use gi_ui::size::SizePolicy;
use gi_ui::canvas::Canvas;
use gi_ui::components::circle::Circle;
use gi_ui::draw::Draw;
use gi_ui::Drawable;
use gi_ui_app::Application;
use x11rb::connection::Connection;

fn create_ui() -> impl Drawable {
    let mut circle = Circle::new()
        .with_radius(SizePolicy::FillParent)
        .set_foreground_color(0xff_ff0000);

    circle
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ui = create_ui();
    let mut application = Application::new(200, 200)?;

    application.attach_main_drawable(&mut ui);

    application.run()
}
