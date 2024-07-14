use uvc_sys::*;

use crate::device::DeviceHandle;
use crate::error::{Error, Result};
use crate::frame::Frame;

use std::os::raw::c_void;

unsafe impl<'a> Send for StreamHandle<'a> {}
unsafe impl<'a> Sync for StreamHandle<'a> {}
#[derive(Debug)]
/// Stream handle
pub struct StreamHandle<'a> {
    pub(crate) handle: uvc_stream_ctrl_t,
    pub(crate) still_handle: Option<uvc_still_ctrl_t>,
    pub(crate) devh: &'a DeviceHandle<'a>,
}

struct Vtable<U> {
    func: Box<dyn Fn(&Frame, &mut U)>,
    data: U,
}

unsafe impl<'a, U: Send + Sync> Send for ActiveStream<'a, U> {}
unsafe impl<'a, U: Send + Sync> Sync for ActiveStream<'a, U> {}
#[derive(Debug)]
/// Active stream
///
/// Dropping this stream will stop the stream
pub struct ActiveStream<'a, U: Send + Sync> {
    devh: &'a crate::DeviceHandle<'a>,
    pub(crate) still_handle: Option<uvc_still_ctrl_t>,
    #[allow(unused)]
    vtable: *mut Vtable<U>,
}

impl<'a, U: Send + Sync> ActiveStream<'a, U> {
    /// Stop the stream
    pub fn stop(self) {
        // Taking ownership of the stream, which drops it
    }
    pub fn trigger_still(&mut self) -> Result<u8> {
        unsafe {
            let err = uvc_trigger_still(
                self.devh.devh.as_ptr(),
                &mut self.still_handle.unwrap(),
            )
            .into();
            if err == Error::Success {
                Ok(0)
            } else {
                Err(err)
            }
        }
    }
}

impl<'a, U: Send + Sync> Drop for ActiveStream<'a, U> {
    fn drop(&mut self) {
        unsafe {
            uvc_stop_streaming(self.devh.devh.as_ptr());
            let _vtable = Box::from_raw(self.vtable);
        }
    }
}

unsafe extern "C" fn trampoline<F, U>(frame: *mut uvc_frame, userdata: *mut c_void)
where
    F: 'static + Send + Sync + Fn(&Frame, &mut U),
    U: 'static + Send + Sync,
{
    let panic = std::panic::catch_unwind(|| {
        if frame.is_null() {
            panic!("Frame is null");
        }
        let frame = std::mem::ManuallyDrop::new(Frame::from_raw(frame));

        if userdata.is_null() {
            panic!("Userdata is null");
        }

        let vtable = userdata as *mut Vtable<U>;

        let func = &(*vtable).func;
        let data = &mut (*vtable).data;

        func(&frame, data);
    });

    if panic.is_err() {
        eprintln!("User defined function panicked");
        std::process::abort();
    }
}

impl<'a> StreamHandle<'a> {
    /// Begin a stream, use the callback to save the frames
    ///
    /// This function is non-blocking
    pub fn start_stream<F, U>(&'a mut self, cb: F, user_data: U) -> Result<ActiveStream<'a, U>>
    where
        F: 'static + Send + Sync + Fn(&Frame, &mut U),
        U: 'static + Send + Sync,
    {
        let tuple = Box::new(Vtable::<U> {
            func: Box::new(cb),
            data: user_data,
        });

        let tuple = Box::into_raw(tuple);

        unsafe {
            let err = uvc_start_streaming(
                self.devh.devh.as_ptr(),
                &mut self.handle,
                Some(trampoline::<F, U>),
                tuple as *mut c_void,
                0,
            )
            .into();
            if err == Error::Success {
                Ok(ActiveStream {
                    devh: self.devh,
                    still_handle: self.still_handle,
                    vtable: tuple,
                })
            } else {
                Err(err)
            }
        }
    }
    pub fn get_still_ctrl_format_size(
        &'a mut self,
        width: u32,
        height: u32,
    ) -> Result<StreamHandle<'a>>
    {
        unsafe {
            let mut still_handle = std::mem::MaybeUninit::uninit();
            let err = uvc_get_still_ctrl_format_size(
                self.devh.devh.as_ptr(),
                &mut self.handle,
                still_handle.as_mut_ptr(),
                width as i32,
                height as i32,
            )
            .into();
            if err == Error::Success {
                Ok(StreamHandle {
                    handle: self.handle,
                    still_handle: Some(still_handle.assume_init()),
                    devh: self.devh,
                })
            } else {
                Err(err)
            }
        }
    }
}
