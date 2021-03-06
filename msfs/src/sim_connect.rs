#![allow(clippy::too_many_arguments)]

use crate::sys;
use std::ptr::NonNull;

pub use sys::SIMCONNECT_OBJECT_ID_USER;

pub use msfs_derive::sim_connect_data_definition as data_definition;

/// A trait implemented by the `data_definition` attribute.
pub trait DataDefinition {
    #[doc(hidden)]
    const DEFINITIONS: &'static [(&'static str, &'static str, sys::SIMCONNECT_DATATYPE)];
}

/// Rusty HRESULT wrapper.
#[derive(Debug)]
pub struct HResult(sys::HRESULT);
impl std::fmt::Display for HResult {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, fmt)
    }
}
impl std::error::Error for HResult {}

pub type Result<T> = std::result::Result<T, HResult>;
#[inline(always)]
fn map_err(result: sys::HRESULT) -> Result<()> {
    if result >= 0 {
        Ok(())
    } else {
        Err(HResult(result))
    }
}

/// Callback provided to SimConnect session.
pub type SimConnectRecvCallback = dyn Fn(&SimConnect, SimConnectRecv);

/// A SimConnect session. This provides access to data within the MSFS sim.
pub struct SimConnect {
    handle: NonNull<std::ffi::c_void>,
    callback: Box<SimConnectRecvCallback>,
}

extern "C" fn dispatch_cb(
    recv: *mut sys::SIMCONNECT_RECV,
    _cb_data: sys::DWORD,
    p_context: *mut std::ffi::c_void,
) {
    let sim = unsafe { &*(p_context as *mut SimConnect) };
    let recv = unsafe {
        match (*recv).dwID as sys::SIMCONNECT_RECV_ID {
            sys::SIMCONNECT_RECV_ID_SIMCONNECT_RECV_ID_NULL => Some(SimConnectRecv::Null),
            sys::SIMCONNECT_RECV_ID_SIMCONNECT_RECV_ID_OPEN => Some(SimConnectRecv::Open(
                &*(recv as *mut sys::SIMCONNECT_RECV_OPEN),
            )),
            sys::SIMCONNECT_RECV_ID_SIMCONNECT_RECV_ID_QUIT => Some(SimConnectRecv::Quit(
                &*(recv as *mut sys::SIMCONNECT_RECV_QUIT),
            )),
            sys::SIMCONNECT_RECV_ID_SIMCONNECT_RECV_ID_EVENT => Some(SimConnectRecv::Event(
                &*(recv as *mut sys::SIMCONNECT_RECV_EVENT),
            )),
            _ => None,
        }
    };
    if let Some(recv) = recv {
        (sim.callback)(sim, recv);
    }
}

impl SimConnect {
    /// Send a request to the Microsoft Flight Simulator server to open up communications with a new client.
    pub fn open<F>(name: &str, callback: F) -> Result<SimConnect>
    where
        F: Fn(&SimConnect, SimConnectRecv) + 'static,
    {
        unsafe {
            let mut ptr = 0;
            let name = std::ffi::CString::new(name).unwrap();
            map_err(sys::SimConnect_Open(
                &mut ptr,
                name.as_ptr(),
                std::ptr::null_mut(),
                0,
                0,
                0,
            ))?;
            let ptr = ptr as *mut std::ffi::c_void;
            debug_assert!(!ptr.is_null());
            let mut sim = SimConnect {
                handle: NonNull::new_unchecked(ptr),
                callback: Box::new(callback),
            };
            sim.call_dispatch()?;
            Ok(sim)
        }
    }

    fn call_dispatch(&mut self) -> Result<()> {
        unsafe {
            map_err(sys::SimConnect_CallDispatch(
                self.handle.as_ptr() as sys::HANDLE,
                Some(dispatch_cb),
                self as *mut SimConnect as *mut std::ffi::c_void,
            ))
        }
    }

    /// Add an individual client defined event to a notification group.
    pub fn add_client_event_to_notification_group(
        &self,
        group_id: sys::SIMCONNECT_NOTIFICATION_GROUP_ID,
        event_id: sys::SIMCONNECT_CLIENT_EVENT_ID,
        maskable: bool,
    ) -> Result<()> {
        unsafe {
            map_err(sys::SimConnect_AddClientEventToNotificationGroup(
                self.handle.as_ptr() as sys::HANDLE,
                group_id,
                event_id,
                maskable as i32,
            ))
        }
    }

    /// Associate a client defined event ID with a Prepar3D event name.
    pub fn map_client_event_to_sim_event(
        &self,
        id: sys::SIMCONNECT_CLIENT_EVENT_ID,
        name: &str,
    ) -> Result<()> {
        unsafe {
            let name = std::ffi::CString::new(name).unwrap();
            map_err(sys::SimConnect_MapClientEventToSimEvent(
                self.handle.as_ptr() as sys::HANDLE,
                id,
                name.as_ptr(),
            ))
        }
    }

    /// Connect an input event (such as a keystroke, joystick or mouse movement) with the sending of an appropriate event notification.
    pub fn map_input_event_to_client_event(
        &self,
        group_id: sys::SIMCONNECT_NOTIFICATION_GROUP_ID,
        input_definition: &str,
        down_event_id: sys::SIMCONNECT_CLIENT_EVENT_ID,
        down_value: sys::DWORD,
        up_event_id: sys::SIMCONNECT_CLIENT_EVENT_ID,
        up_value: sys::DWORD,
        maskable: bool,
    ) -> Result<()> {
        unsafe {
            let input_definition = std::ffi::CString::new(input_definition).unwrap();
            map_err(sys::SimConnect_MapInputEventToClientEvent(
                self.handle.as_ptr() as sys::HANDLE,
                group_id,
                input_definition.as_ptr(),
                down_event_id,
                down_value,
                up_event_id,
                up_value,
                maskable as i32,
            ))
        }
    }

    /// Set the priority for a notification group.
    pub fn set_notification_group_priority(
        &self,
        group_id: sys::SIMCONNECT_NOTIFICATION_GROUP_ID,
        priority: sys::DWORD,
    ) -> Result<()> {
        unsafe {
            map_err(sys::SimConnect_SetNotificationGroupPriority(
                self.handle.as_ptr() as sys::HANDLE,
                group_id,
                priority,
            ))
        }
    }

    /// Remove a client defined event from a notification group.
    pub fn remove_client_event(
        &self,
        group_id: sys::SIMCONNECT_NOTIFICATION_GROUP_ID,
        event_id: sys::SIMCONNECT_CLIENT_EVENT_ID,
    ) -> Result<()> {
        unsafe {
            map_err(sys::SimConnect_RemoveClientEvent(
                self.handle.as_ptr() as sys::HANDLE,
                group_id,
                event_id,
            ))
        }
    }

    /// Associate a data definition with a client defined object definition.
    pub fn add_data_definition<T: DataDefinition>(
        &self,
        define_id: sys::SIMCONNECT_DATA_DEFINITION_ID,
    ) -> Result<()> {
        for (datum_name, units_type, datatype) in T::DEFINITIONS {
            let datum_name = std::ffi::CString::new(*datum_name).unwrap();
            let units_type = std::ffi::CString::new(*units_type).unwrap();
            unsafe {
                map_err(sys::SimConnect_AddToDataDefinition(
                    self.handle.as_ptr() as sys::HANDLE,
                    define_id,
                    datum_name.as_ptr(),
                    units_type.as_ptr(),
                    *datatype,
                    0.0,
                    0,
                ))?;
            }
        }
        Ok(())
    }

    /// Make changes to the data properties of an object.
    pub fn set_data_on_sim_object<T: DataDefinition>(
        &self,
        define_id: sys::SIMCONNECT_DATA_DEFINITION_ID,
        object_id: sys::SIMCONNECT_OBJECT_ID,
        data: &T,
    ) -> Result<()> {
        unsafe {
            map_err(sys::SimConnect_SetDataOnSimObject(
                self.handle.as_ptr() as sys::HANDLE,
                define_id,
                object_id,
                0,
                0,
                std::mem::size_of_val(data) as sys::DWORD,
                data as *const T as *mut std::ffi::c_void,
            ))
        }
    }
}

/// Message received from `SimConnect::get_next_dispatch`.
#[derive(Debug)]
pub enum SimConnectRecv<'a> {
    Null,
    Exception(&'a sys::SIMCONNECT_RECV_EXCEPTION),
    Open(&'a sys::SIMCONNECT_RECV_OPEN),
    Quit(&'a sys::SIMCONNECT_RECV_QUIT),
    Event(&'a sys::SIMCONNECT_RECV_EVENT),
}

impl Drop for SimConnect {
    fn drop(&mut self) {
        assert!(unsafe { sys::SimConnect_Close(self.handle.as_ptr() as sys::HANDLE) } >= 0);
    }
}
