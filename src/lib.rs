use gi_ui::{Drawable, canvas::Canvas};
use x11rb::{
    connection::Connection, protocol::{
        xproto::{
            BackingStore, ChangeWindowAttributesAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, Pixmap, Screen, Window, WindowClass
        }, Event
    }, rust_connection::RustConnection, wrapper::ConnectionExt as WrappedConnectionExt, COPY_DEPTH_FROM_PARENT
};

pub struct Application<'a> {
    canvas: Canvas,
    main_drawable: Option<&'a mut dyn Drawable>,

    // X11 zone
    conn: RustConnection,
    screen_num: usize,

    window_id: Window,

    pixmap_id: Pixmap,
    gc_id: u32,
}

impl<'drawable> Application<'drawable> {
    pub fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;

        let canvas = Canvas::new(width as _, height as _);
        let win = conn.generate_id()?;

        let screen = &conn.setup().roots[screen_num];

        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            screen.root,
            0,
            0, // x, y
            width as _,
            height as _, // width, height
            0,           // border width
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                // .background_pixel(screen.white_pixel)
                .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY),
        )?;

        conn.map_window(win)?;
        conn.flush()?;

        let pixmap = conn.generate_id()?;
        conn.create_pixmap(
            screen.root_depth,
            pixmap,
            screen.root,
            width as _,
            height as _,
        )?
        .check()?;

        let gc = conn.generate_id()?;
        conn.create_gc(gc, pixmap, &CreateGCAux::new())?.check()?;

        Ok(Application {
            canvas,
            main_drawable: None,
            conn,
            screen_num,
            window_id: win,
            pixmap_id: pixmap,
            gc_id: gc,
        })
    }

    fn recreate_pixmap(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let screen = &self.conn.setup().roots[self.screen_num];

        self.conn.free_pixmap(self.pixmap_id)?;
        self.conn.free_gc(self.gc_id)?;

        self.pixmap_id = self.conn.generate_id()?;
        self.gc_id = self.conn.generate_id()?;

        self.conn
            .create_pixmap(
                screen.root_depth,
                self.pixmap_id,
                screen.root,
                self.canvas.width() as _,
                self.canvas.height() as _,
            )?
            .check()?;

        self.conn.create_gc(self.gc_id, self.pixmap_id, &CreateGCAux::new())?.check()?;

        Ok(())
    }

    fn draw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let screen = &self.conn.setup().roots[self.screen_num];

        if let Some(drawable) = &mut self.main_drawable {
            drawable.draw(&mut self.canvas, 0, 0);
        }

        let buffer = self.canvas.buffer();
        let width = self.canvas.width();
        let height = self.canvas.height();

        let mut x11_buffer = vec![0; buffer.len()];
        let cycles = width * height;
        for i in 0..cycles {
            let argb = &buffer[i * 4..(i + 1) * 4];

            x11_buffer[i * 4] = argb[2];
            x11_buffer[i * 4 + 1] = argb[1];
            x11_buffer[i * 4 + 2] = argb[0];
            x11_buffer[i * 4 + 3] = argb[3];
        }

        self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.pixmap_id,
            self.gc_id,
            width as _,
            height as _,
            0,
            0,
            0,
            screen.root_depth,
            &x11_buffer,
        )?.check()?;

        self.conn
            .copy_area(
                self.pixmap_id,
                self.window_id,
                self.gc_id,
                0,
                0, // src x, y
                0,
                0, // dst x, y
                width as _,
                height as _,
            )?
            .check()?;

        Ok(())
    }

    pub fn attach_main_drawable(&mut self, drawable: &'drawable mut dyn Drawable) {
        self.main_drawable = Some(drawable);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let event = self.conn.wait_for_event()?;
            match event {
                Event::Expose(ev) => {
                    // Handle redraw when window is exposed

                    if ev.count == 0 {
                        self.draw()?;
                    }
                }
                Event::ConfigureNotify(msg) => {
                    // Handle window resize
                    
                    self.canvas.fill(0);
                    self.canvas.resize(msg.width as _, msg.height as _);
                    self.recreate_pixmap()?;
                }
                Event::DestroyNotify(_) => {
                    println!("Destroy!");
                    break;
                }
                _ => {}
            }

            self.conn.sync()?;
            self.conn.flush()?;
        }

        Ok(())
    }
}

impl Drop for Application<'_> {
    fn drop(&mut self) {
        self.conn.free_gc(self.gc_id).unwrap();
        self.conn.free_pixmap(self.pixmap_id).unwrap();
    }
}
