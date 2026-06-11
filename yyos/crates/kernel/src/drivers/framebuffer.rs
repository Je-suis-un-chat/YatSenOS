use core::{convert::Infallible, fmt, ptr};

use boot::{FrameBufferInfo, FrameBufferPixelFormat};
use embedded_graphics::{
    Drawable,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_8X13},
    pixelcolor::{Rgb888, RgbColor},
    prelude::{Pixel, Primitive},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

const BYTES_PER_PIXEL: usize = 4;
const GLYPH_WIDTH: usize = 8;
const GLYPH_HEIGHT: usize = 13;
const LINE_HEIGHT: usize = 15;
const MARGIN: usize = 8;
const BACKGROUND: Rgb888 = Rgb888::new(8, 13, 24);
const PANEL_BACKGROUND: Rgb888 = Rgb888::new(13, 24, 42);
const FOREGROUND: Rgb888 = Rgb888::new(226, 232, 240);
const ACCENT: Rgb888 = Rgb888::new(56, 189, 248);

pub static FRAMEBUFFER: spin::Once<spin::Mutex<FrameBufferConsole>> = spin::Once::new();

pub fn init(info: Option<FrameBufferInfo>, physical_memory_offset: u64) {
    let Some(info) = info else {
        return;
    };
    if info.pixel_format == FrameBufferPixelFormat::Unknown || info.width == 0 || info.height == 0 {
        return;
    }

    let address = info.address.saturating_add(physical_memory_offset) as *mut u8;
    let mut console = unsafe { FrameBufferConsole::new(address, info) };
    console.initialize_screen();
    FRAMEBUFFER.call_once(|| spin::Mutex::new(console));
}

pub fn write_fmt(args: fmt::Arguments<'_>) {
    if let Some(framebuffer) = FRAMEBUFFER.get() {
        if let Some(mut console) = framebuffer.try_lock() {
            let _ = fmt::Write::write_fmt(&mut *console, args);
        }
    }
}

pub fn update_clock(ticks: u64) {
    if ticks % 64 != 0 {
        return;
    }
    if let Some(framebuffer) = FRAMEBUFFER.get() {
        if let Some(mut console) = framebuffer.try_lock() {
            console.draw_clock(ticks / 64);
        }
    }
}

pub struct FrameBufferConsole {
    address: *mut u8,
    info: FrameBufferInfo,
    terminal_top: usize,
    cursor_x: usize,
    cursor_y: usize,
    foreground: Rgb888,
    state: EscapeState,
}

unsafe impl Send for FrameBufferConsole {}

#[derive(Clone, Copy)]
enum EscapeState {
    Normal,
    Escape,
    Csi { value: usize, has_value: bool },
}

impl FrameBufferConsole {
    unsafe fn new(address: *mut u8, info: FrameBufferInfo) -> Self {
        let terminal_top = if info.height >= 400 {
            info.height / 2
        } else {
            0
        };
        Self {
            address,
            info,
            terminal_top,
            cursor_x: MARGIN,
            cursor_y: terminal_top + MARGIN,
            foreground: FOREGROUND,
            state: EscapeState::Normal,
        }
    }

    fn initialize_screen(&mut self) {
        self.fill_region(0, 0, self.info.width, self.info.height, BACKGROUND);
        if self.terminal_top > 0 {
            self.fill_region(0, 0, self.info.width, self.terminal_top, PANEL_BACKGROUND);
            let style = MonoTextStyle::new(&FONT_8X13, ACCENT);
            let _ = Text::with_baseline(
                "YatSenOS graphical console",
                Point::new(MARGIN as i32, MARGIN as i32),
                style,
                Baseline::Top,
            )
            .draw(self);
            self.draw_clock(0);
        }
        self.clear_terminal();
    }

    fn write_char(&mut self, ch: char) {
        match self.state {
            EscapeState::Normal => match ch {
                '\x1b' => self.state = EscapeState::Escape,
                '\n' => self.new_line(),
                '\r' => self.cursor_x = MARGIN,
                '\x08' => self.backspace(),
                c if !c.is_control() => self.draw_terminal_char(c),
                _ => {}
            },
            EscapeState::Escape => {
                self.state = if ch == '[' {
                    EscapeState::Csi {
                        value: 0,
                        has_value: false,
                    }
                } else {
                    EscapeState::Normal
                };
            }
            EscapeState::Csi {
                mut value,
                mut has_value,
            } => {
                if let Some(digit) = ch.to_digit(10) {
                    value = value.saturating_mul(10).saturating_add(digit as usize);
                    has_value = true;
                    self.state = EscapeState::Csi { value, has_value };
                    return;
                }
                match ch {
                    'J' if !has_value || value == 2 => self.clear_terminal(),
                    'H' => {
                        self.cursor_x = MARGIN;
                        self.cursor_y = self.terminal_top + MARGIN;
                    }
                    'm' => self.foreground = ansi_color(if has_value { value } else { 0 }),
                    ';' => {
                        self.state = EscapeState::Csi {
                            value: 0,
                            has_value: false,
                        };
                        return;
                    }
                    _ => {}
                }
                self.state = EscapeState::Normal;
            }
        }
    }

    fn draw_terminal_char(&mut self, ch: char) {
        if self.cursor_x + GLYPH_WIDTH + MARGIN > self.info.width {
            self.new_line();
        }

        let cell = Rectangle::new(
            Point::new(self.cursor_x as i32, self.cursor_y as i32),
            Size::new(GLYPH_WIDTH as u32, GLYPH_HEIGHT as u32),
        );
        let _ = cell
            .into_styled(PrimitiveStyle::with_fill(BACKGROUND))
            .draw(self);

        let mut encoded = [0u8; 4];
        let text = if ch.is_ascii() {
            ch.encode_utf8(&mut encoded)
        } else {
            "?"
        };
        let style = MonoTextStyle::new(&FONT_8X13, self.foreground);
        let _ = Text::with_baseline(
            text,
            Point::new(self.cursor_x as i32, self.cursor_y as i32),
            style,
            Baseline::Top,
        )
        .draw(self);
        self.cursor_x += GLYPH_WIDTH;
    }

    fn new_line(&mut self) {
        self.cursor_x = MARGIN;
        self.cursor_y += LINE_HEIGHT;
        if self.cursor_y + LINE_HEIGHT + MARGIN > self.info.height {
            self.scroll();
            self.cursor_y = self.info.height.saturating_sub(LINE_HEIGHT + MARGIN);
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x > MARGIN {
            self.cursor_x -= GLYPH_WIDTH;
            self.fill_region(
                self.cursor_x,
                self.cursor_y,
                GLYPH_WIDTH,
                GLYPH_HEIGHT,
                BACKGROUND,
            );
        }
    }

    fn clear_terminal(&mut self) {
        self.fill_region(
            0,
            self.terminal_top,
            self.info.width,
            self.info.height - self.terminal_top,
            BACKGROUND,
        );
        if self.terminal_top > 0 {
            self.fill_region(0, self.terminal_top, self.info.width, 1, ACCENT);
        }
        self.cursor_x = MARGIN;
        self.cursor_y = self.terminal_top + MARGIN;
    }

    fn scroll(&mut self) {
        let start = self.terminal_top + MARGIN;
        let end = self.info.height.saturating_sub(MARGIN);
        if end <= start + LINE_HEIGHT {
            return;
        }
        let row_bytes = self.info.stride * BYTES_PER_PIXEL;
        for y in start..end - LINE_HEIGHT {
            let dst = y * row_bytes;
            let src = (y + LINE_HEIGHT) * row_bytes;
            for x in MARGIN * BYTES_PER_PIXEL..(self.info.width - MARGIN) * BYTES_PER_PIXEL {
                unsafe {
                    let value = ptr::read_volatile(self.address.add(src + x));
                    ptr::write_volatile(self.address.add(dst + x), value);
                }
            }
        }
        self.fill_region(
            MARGIN,
            end - LINE_HEIGHT,
            self.info.width.saturating_sub(MARGIN * 2),
            LINE_HEIGHT,
            BACKGROUND,
        );
    }

    fn draw_clock(&mut self, seconds: u64) {
        if self.terminal_top == 0 {
            return;
        }
        let radius = (self.terminal_top.min(self.info.width) / 3).clamp(36, 110) as i32;
        let center = Point::new(
            (self.info.width / 2) as i32,
            (self.terminal_top / 2) as i32 + 4,
        );
        let area = Rectangle::new(
            Point::new(center.x - radius - 5, center.y - radius - 5),
            Size::new((radius * 2 + 10) as u32, (radius * 2 + 10) as u32),
        );
        let _ = area
            .into_styled(PrimitiveStyle::with_fill(PANEL_BACKGROUND))
            .draw(self);
        let circle = Circle::with_center(center, (radius * 2) as u32)
            .into_styled(PrimitiveStyle::with_stroke(ACCENT, 2));
        let _ = circle.draw(self);

        let angle =
            (seconds % 60) as f64 * core::f64::consts::TAU / 60.0 - core::f64::consts::FRAC_PI_2;
        let end = Point::new(
            center.x + (libm::cos(angle) * (radius - 8) as f64) as i32,
            center.y + (libm::sin(angle) * (radius - 8) as f64) as i32,
        );
        let _ = Line::new(center, end)
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::new(248, 113, 113), 3))
            .draw(self);
    }

    fn fill_region(&mut self, x: usize, y: usize, width: usize, height: usize, color: Rgb888) {
        let x_end = x.saturating_add(width).min(self.info.width);
        let y_end = y.saturating_add(height).min(self.info.height);
        for py in y..y_end {
            for px in x..x_end {
                self.put_pixel(px, py, color);
            }
        }
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let offset = (y * self.info.stride + x) * BYTES_PER_PIXEL;
        if offset + 3 >= self.info.size {
            return;
        }
        let bytes = match self.info.pixel_format {
            FrameBufferPixelFormat::Rgb => [color.r(), color.g(), color.b(), 0],
            FrameBufferPixelFormat::Bgr => [color.b(), color.g(), color.r(), 0],
            FrameBufferPixelFormat::Unknown => return,
        };
        for (index, byte) in bytes.into_iter().enumerate() {
            unsafe { ptr::write_volatile(self.address.add(offset + index), byte) };
        }
    }
}

impl fmt::Write for FrameBufferConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            self.write_char(ch);
        }
        Ok(())
    }
}

impl OriginDimensions for FrameBufferConsole {
    fn size(&self) -> Size {
        Size::new(self.info.width as u32, self.info.height as u32)
    }
}

impl DrawTarget for FrameBufferConsole {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 {
                self.put_pixel(point.x as usize, point.y as usize, color);
            }
        }
        Ok(())
    }
}

fn ansi_color(code: usize) -> Rgb888 {
    match code {
        31 => Rgb888::new(248, 113, 113),
        32 => Rgb888::new(74, 222, 128),
        33 => Rgb888::new(250, 204, 21),
        34 => Rgb888::new(96, 165, 250),
        35 => Rgb888::new(216, 180, 254),
        36 => Rgb888::new(34, 211, 238),
        90 => Rgb888::new(148, 163, 184),
        _ => FOREGROUND,
    }
}
