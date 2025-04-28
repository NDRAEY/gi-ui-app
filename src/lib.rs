use std::{cell::{Cell, RefCell}, rc::Rc};

use gi_ui::{Drawable, canvas::Canvas};
use x11rb::{
    COPY_DEPTH_FROM_PARENT,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            AtomEnum, ConfigureWindowAux, ConnectionExt,
            CreateGCAux, CreateWindowAux, EventMask, ImageFormat, Pixmap, PropMode, Window,
            WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as WrappedConnectionExt,
};

const DEFAULT_TITLE: &str = "Untitled";

pub struct Application {
    canvas: RefCell<Canvas>,
    main_drawable: Option<Rc<RefCell<Box<dyn Drawable>>>>,

    title: RefCell<String>,

    width: u32,
    height: u32,

    pub on_resize_callback: Option<Box<dyn Fn(usize, usize)>>,

    // X11 zone
    conn: RustConnection,
    screen_num: usize,

    window_id: Window,

    pixmap_id: Pixmap,
    gc_id: u32,
}

impl Application {
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

        let mut app = Application {
            title: RefCell::new(String::from(DEFAULT_TITLE)),
            canvas: RefCell::new(canvas),
            main_drawable: None,

            width,
            height,

            on_resize_callback: None,

            // X11
            conn,
            screen_num,
            window_id: win,
            pixmap_id: pixmap,
            gc_id: gc,
        };

        app.set_title(DEFAULT_TITLE)?;

        Ok(app)
    }

    fn recreate_pixmap(&self) -> Result<(), Box<dyn std::error::Error>> {
        let screen = &self.conn.setup().roots[self.screen_num];

        self.conn.free_pixmap(self.pixmap_id)?;
        self.conn.free_gc(self.gc_id)?;

        // self.pixmap_id = self.conn.generate_id()?;
        // self.gc_id = self.conn.generate_id()?;

        let canvas = self.canvas.borrow();

        self.conn
            .create_pixmap(
                screen.root_depth,
                self.pixmap_id,
                screen.root,
                canvas.width() as _,
                canvas.height() as _,
            )?
            .check()?;

        self.conn
            .create_gc(self.gc_id, self.pixmap_id, &CreateGCAux::new())?
            .check()?;

        Ok(())
    }

    fn draw(&self) -> Result<(), Box<dyn std::error::Error>> {
        let screen = &self.conn.setup().roots[self.screen_num];

        if let Some(drawable) = &self.main_drawable {
            let mut canv = self.canvas.borrow_mut();
            drawable.borrow_mut().draw(&mut canv, 0, 0);
        }

        let canv = self.canvas.borrow();

        let buffer = canv.buffer();
        let width = canv.width();
        let height = canv.height();

        let mut x11_buffer = vec![0; buffer.len()];
        let cycles = width * height;
        for i in 0..cycles {
            let argb = &buffer[i * 4..(i + 1) * 4];

            x11_buffer[i * 4] = argb[2];
            x11_buffer[i * 4 + 1] = argb[1];
            x11_buffer[i * 4 + 2] = argb[0];
            x11_buffer[i * 4 + 3] = argb[3];
        }

        self.conn
            .put_image(
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
            )?
            .check()?;

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

    pub fn attach_main_drawable(
        &mut self,
        drawable: Box<dyn Drawable>,
    ) -> &Rc<RefCell<Box<dyn Drawable>>> {
        self.main_drawable = Some(Rc::new(RefCell::new(drawable)));

        self.main_drawable.as_ref().unwrap()
    }

    pub fn hide(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.unmap_window(self.window_id)?;
        self.conn.flush()?;

        Ok(())
    }

    pub fn show(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.map_window(self.window_id)?;
        self.conn.flush()?;

        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.configure_window(
            self.window_id,
            &ConfigureWindowAux::default().width(width).height(height),
        )?;

        Ok(())
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn set_position(&self, x: i32, y: i32) -> Result<(), Box<dyn std::error::Error>> {
        self.conn
            .configure_window(self.window_id, &ConfigureWindowAux::default().x(x).y(y))?;

        Ok(())
    }

    pub fn set_resize_callback(&mut self, cb: (impl Fn(usize, usize) + 'static)) {
        self.on_resize_callback = Some(Box::new(cb));
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.show()?;

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

                    {
                        let mut canv = self.canvas.borrow_mut();

                        canv.fill(0);
                        canv.resize(msg.width as _, msg.height as _);
                    }
                    
                    self.recreate_pixmap()?;

                    if self.on_resize_callback.is_some() {
                        (self.on_resize_callback.as_ref().unwrap())(msg.width as _, msg.height as _);
                    }
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

    pub fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub fn set_title<S: ToString>(&self, title: S) -> Result<(), Box<dyn std::error::Error>> {
        let t = title.to_string();

        self.conn.change_property8(
            PropMode::REPLACE,
            self.window_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            t.as_bytes(),
        )?;
        
        *self.title.borrow_mut() = t;
        
        Ok(())
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.conn.free_gc(self.gc_id).unwrap();
        self.conn.free_pixmap(self.pixmap_id).unwrap();
    }
}
