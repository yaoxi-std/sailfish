//! Build-in filters

// TODO: performance improvement

use std::fmt;
use std::ptr;

use super::{Buffer, Render, RenderError};

pub struct Display<'a, T>(&'a T);

impl<'a, T: fmt::Display> Render for Display<'a, T> {
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        use fmt::Write;

        write!(b, "{}", self.0).map_err(|e| RenderError::from(e))
    }
}

/// render using `std::fmt::Display` trait
#[inline]
pub fn disp<T: fmt::Display>(expr: &T) -> Display<T> {
    Display(expr)
}

pub struct Debug<'a, T>(&'a T);

impl<'a, T: fmt::Debug> Render for Debug<'a, T> {
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        use fmt::Write;

        write!(b, "{:?}", self.0).map_err(|e| RenderError::from(e))
    }
}

/// render using `std::fmt::Debug` trait
#[inline]
pub fn dbg<T: fmt::Debug>(expr: &T) -> Debug<T> {
    Debug(expr)
}

pub struct Upper<'a, T>(&'a T);

impl<'a, T: Render> Render for Upper<'a, T> {
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render(b)?;

        let s = b.as_str()[old_len..].to_uppercase();
        unsafe { b._set_len(old_len) };
        b.push_str(&*s);
        Ok(())
    }
}

/// convert the rendered contents to uppercase
#[inline]
pub fn upper<T: Render>(expr: &T) -> Upper<T> {
    Upper(expr)
}

pub struct Lower<'a, T>(&'a T);

impl<'a, T: Render> Render for Lower<'a, T> {
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render(b)?;

        let s = b.as_str()[old_len..].to_lowercase();
        unsafe { b._set_len(old_len) };
        b.push_str(&*s);
        Ok(())
    }

    fn render_escaped(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render_escaped(b)?;

        let s = b.as_str()[old_len..].to_lowercase();
        unsafe { b._set_len(old_len) };
        b.push_str(&*s);
        Ok(())
    }
}

/// convert the rendered contents to lowercase
#[inline]
pub fn lower<T: Render>(expr: &T) -> Lower<T> {
    Lower(expr)
}

pub struct Trim<'a, T>(&'a T);

impl<'a, T: Render> Render for Trim<'a, T> {
    #[inline]
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render(b)?;
        trim_impl(b, old_len);
        Ok(())
    }

    #[inline]
    fn render_escaped(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render_escaped(b)?;
        trim_impl(b, old_len);
        Ok(())
    }
}

fn trim_impl(b: &mut Buffer, old_len: usize) {
    debug_assert!(b.len() >= old_len);
    let new_contents = &b.as_str()[old_len..];
    let trimmed = new_contents.trim();
    let trimmed_len = trimmed.len();

    if new_contents.len() != trimmed_len {
        // performs inplace trimming

        if new_contents.as_ptr() != trimmed.as_ptr() {
            debug_assert!(new_contents.as_ptr() < trimmed.as_ptr());
            let offset = trimmed.as_ptr() as usize - new_contents.as_ptr() as usize;
            unsafe {
                ptr::copy(
                    b.as_mut_ptr().add(old_len + offset),
                    b.as_mut_ptr().add(old_len),
                    trimmed_len,
                );
            }
        }

        debug_assert!(b.capacity() >= old_len + trimmed_len);

        // SAFETY: `new_contents.len() = b.len() - old_len` and
        // `trimmed_len < new_contents.len()`, so `old_len + trimmed_len < b.len()`.
        unsafe {
            b._set_len(old_len + trimmed_len);
        }
    }
}

/// Remove leading and trailing writespaces from rendered results
#[inline]
pub fn trim<T: Render>(expr: &T) -> Trim<T> {
    Trim(expr)
}

pub struct Truncate<'a, T>(&'a T, usize);

impl<'a, T: Render> Render for Truncate<'a, T> {
    #[inline]
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render(b)?;
        truncate_impl(b, old_len, self.1)
    }

    #[inline]
    fn render_escaped(&self, b: &mut Buffer) -> Result<(), RenderError> {
        let old_len = b.len();
        self.0.render_escaped(b)?;
        truncate_impl(b, old_len, self.1)
    }
}

fn truncate_impl(
    b: &mut Buffer,
    old_len: usize,
    limit: usize,
) -> Result<(), RenderError> {
    let mut pos = old_len + limit;
    if b.len() > pos {
        let tmp = b.as_str();
        while !tmp.is_char_boundary(pos) {
            pos += 1;
        }

        unsafe { b._set_len(pos) };
        b.push_str("...");

        Ok(())
    } else if b.len() >= old_len {
        Ok(())
    } else {
        Err(RenderError::new("buffer size shrinked while rendering"))
    }
}

/// Limit length of rendered contents, appends '...' if truncated
#[inline]
pub fn truncate<T: Render>(expr: &T, mut limit: usize) -> Truncate<T> {
    // SAFETY: since `buf.len() <= isize::MAX`, length of rendered contents never
    // overflows isize::MAX. If limit > isize::MAX, then truncation never happens
    limit &= std::usize::MAX >> 1;
    Truncate(expr, limit)
}

cfg_json! {
    pub struct Json<'a, T>(&'a T);

    impl<'a, T: serde::Serialize> Render for Json<'a, T> {
        #[inline]
        fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
            struct Writer<'a>(&'a mut Buffer);

            impl<'a> std::io::Write for Writer<'a> {
                #[inline]
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    let buf = unsafe { std::str::from_utf8_unchecked(buf) };
                    self.0.push_str(buf);
                    Ok(buf.len())
                }

                #[inline]
                fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
                    self.write(buf).map(|_| {})
                }

                #[inline]
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }

            serde_json::to_writer(Writer(b), self.0)
                .map_err(|e| RenderError::new(&e.to_string()))
        }

        #[inline]
        fn render_escaped(&self, b: &mut Buffer) -> Result<(), RenderError> {
            use super::escape::escape_to_buf;

            struct Writer<'a>(&'a mut Buffer);

            impl<'a> std::io::Write for Writer<'a> {
                #[inline]
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    let buf = unsafe { std::str::from_utf8_unchecked(buf) };
                    escape_to_buf(buf, self.0);
                    Ok(buf.len())
                }

                #[inline]
                fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
                    self.write(buf).map(|_| {})
                }

                #[inline]
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }

            serde_json::to_writer(Writer(b), self.0)
                .map_err(|e| RenderError::new(&e.to_string()))
        }
    }

    /// Serialize the given data structure as JSON into the buffer
    #[inline]
    pub fn json<T: serde::Serialize>(expr: &T) -> Json<T> {
        Json(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case() {
        let mut buf = Buffer::new();
        upper(&"hElLO, WOrLd!").render(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "HELLO, WORLD!");

        buf.clear();
        lower(&"hElLO, WOrLd!").render(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "hello, world!");

        buf.clear();
        lower(&"<h1>TITLE</h1>").render_escaped(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "&lt;h1&gt;title&lt;/h1&gt;");
    }

    #[test]
    fn trim_test() {
        let mut buf = Buffer::new();
        trim(&" hello  ").render(&mut buf).unwrap();
        trim(&"hello ").render(&mut buf).unwrap();
        trim(&"   hello").render(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "hellohellohello");

        let mut buf = Buffer::new();
        trim(&"hello ").render(&mut buf).unwrap();
        trim(&" hello").render(&mut buf).unwrap();
        trim(&"hello").render(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "hellohellohello");

        let mut buf = Buffer::new();
        trim(&" hello").render(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "hello");
    }
}
