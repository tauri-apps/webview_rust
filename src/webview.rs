use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::*;
use std::ptr::null_mut;
use std::sync::{Arc, Weak};

use crate::{Error};

use webview_official_sys as sys;

pub enum Window {}

#[repr(i32)]
#[derive(Debug)]
pub enum SizeHint {
    NONE = 0,
    MIN = 1,
    MAX = 2,
    FIXED = 3,
}

impl Default for SizeHint {
    fn default() -> Self {
        SizeHint::NONE
    }
}

pub struct Webview<'a> {
    inner: Arc<sys::webview_t>,
    url: &'a str,
}

unsafe impl Send for Webview<'_> {}
unsafe impl Sync for Webview<'_> {}

impl<'a> Drop for Webview<'a> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 0 {
            unsafe {
                sys::webview_terminate(*self.inner);
                sys::webview_destroy(*self.inner);
            }
        }
    }
}

impl<'a> Webview<'a> {
    pub fn create(debug: bool, window: Option<&mut Window>) -> Webview {
        if let Some(w) = window {
            Webview {
                inner: Arc::new(unsafe {
                    sys::webview_create(debug as c_int, w as *mut Window as *mut _)
                }),
                url: "",
            }
        } else {
            Webview {
                inner: Arc::new(unsafe { sys::webview_create(debug as c_int, null_mut()) }),
                url: "",
            }
        }
    }

    pub fn run(&mut self) {
        let c_url = CString::new(self.url).expect("No null bytes in parameter url");
        unsafe { sys::webview_navigate(*self.inner, c_url.as_ptr()) }
        unsafe { sys::webview_run(*self.inner) }
    }

    pub fn terminate(&mut self) {
        unsafe { sys::webview_terminate(*self.inner) }
    }

    pub fn as_mut(&mut self) -> WebviewMut {
        WebviewMut(Arc::downgrade(&self.inner))
    }

    // TODO Window instance
    pub fn set_title(&mut self, title: &str) {
        let c_title = CString::new(title).expect("No null bytes in parameter title");
        unsafe { sys::webview_set_title(*self.inner, c_title.as_ptr()) }
    }

    pub fn set_size(&mut self, width: i32, height: i32, hints: SizeHint) {
        unsafe { sys::webview_set_size(*self.inner, width, height, hints as i32) }
    }

    pub fn get_window(&self) -> *mut Window {
        unsafe { sys::webview_get_window(*self.inner) as *mut Window }
    }

    pub fn navigate(&mut self, url: &'a str) {
        self.url = url;
    }

    pub fn init(&mut self, js: &str) {
        let c_js = CString::new(js).expect("No null bytes in parameter js");
        unsafe { sys::webview_init(*self.inner, c_js.as_ptr()) }
    }

    pub fn eval(&mut self, js: &str) {
        let c_js = CString::new(js).expect("No null bytes in parameter js");
        unsafe { sys::webview_eval(*self.inner, c_js.as_ptr()) }
    }

    pub fn dispatch<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Webview) + Send + 'static,
    {
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(webview: sys::webview_t, arg: *mut c_void)
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            let mut webview = Webview {
                inner: Arc::new(webview),
                url: "",
            };
            let closure: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*closure)(&mut webview);
        }
        unsafe { sys::webview_dispatch(*self.inner, Some(callback::<F>), closure as *mut _) }
    }

    pub fn bind<F>(&mut self, name: &str, f: F)
    where
        F: FnMut(&str, &str),
    {
        let c_name = CString::new(name).expect("No null bytes in parameter name");
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(seq: *const c_char, req: *const c_char, arg: *mut c_void)
        where
            F: FnMut(&str, &str),
        {
            let seq = unsafe {
                CStr::from_ptr(seq)
                    .to_str()
                    .expect("No null bytes in parameter seq")
            };
            let req = unsafe {
                CStr::from_ptr(req)
                    .to_str()
                    .expect("No null bytes in parameter req")
            };
            let mut f: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*f)(seq, req);
            mem::forget(f);
        }
        unsafe {
            sys::webview_bind(
                *self.inner,
                c_name.as_ptr(),
                Some(callback::<F>),
                closure as *mut _,
            )
        }
    }

    pub fn r#return(&self, seq: &str, status: c_int, result: &str) {
        let c_seq = CString::new(seq).expect("No null bytes in parameter seq");
        let c_result = CString::new(result).expect("No null bytes in parameter result");
        unsafe { sys::webview_return(*self.inner, c_seq.as_ptr(), status, c_result.as_ptr()) }
    }
}

#[derive(Clone)]
pub struct WebviewMut(Weak<sys::webview_t>);

unsafe impl Send for WebviewMut {}
unsafe impl Sync for WebviewMut {}

impl WebviewMut {
    pub fn terminate(&mut self) -> Result<(), Error> {
        let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
        unsafe { sys::webview_terminate(*webview) }
        Ok(())
    }

    pub fn get_window(&self) -> Result<*mut Window, Error> {
        let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
        Ok(unsafe { sys::webview_get_window(*webview) as *mut Window })
    }

    pub fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Webview) + Send + 'static,
    {
        let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(webview: sys::webview_t, arg: *mut c_void)
        where
            F: FnOnce(&mut Webview) + Send + 'static,
        {
            let mut webview = Webview {
                inner: Arc::new(webview),
                url: "",
            };
            let closure: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*closure)(&mut webview);
        }
        unsafe { sys::webview_dispatch(*webview, Some(callback::<F>), closure as *mut _) }
        Ok(())
    }

    pub fn bind<F>(&mut self, name: &str, f: F) -> Result<(), Error>
    where
        F: FnMut(&str, &str) + 'static,
    {
        let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
        let c_name = CString::new(name).expect("No null bytes in parameter name");
        let closure = Box::into_raw(Box::new(f));
        extern "C" fn callback<F>(seq: *const c_char, req: *const c_char, arg: *mut c_void)
        where
            F: FnMut(&str, &str) + 'static,
        {
            let seq = unsafe {
                CStr::from_ptr(seq)
                    .to_str()
                    .expect("No null bytes in parameter seq")
            };
            let req = unsafe {
                CStr::from_ptr(req)
                    .to_str()
                    .expect("No null bytes in parameter req")
            };
            let mut f: Box<F> = unsafe { Box::from_raw(arg as *mut F) };
            (*f)(seq, req);
            mem::forget(f);
        }
        unsafe {
            sys::webview_bind(
                *webview,
                c_name.as_ptr(),
                Some(callback::<F>),
                closure as *mut _,
            )
        }
        Ok(())
    }

    pub fn r#return(&self, seq: &str, status: c_int, result: &str) -> Result<(), Error> {
        let webview = self.0.upgrade().ok_or(Error::WebviewNull)?;
        let c_seq = CString::new(seq).expect("No null bytes in parameter seq");
        let c_result = CString::new(result).expect("No null bytes in parameter result");
        unsafe { sys::webview_return(*webview, c_seq.as_ptr(), status, c_result.as_ptr()) }
        Ok(())
    }
}
