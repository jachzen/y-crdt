//! yffi/src/awarenessw.rs  –  C-ABI wrapper around yrs::sync::Awareness

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;
use yrs::Doc;
use yrs::sync::{Awareness as RawAwareness, AwarenessUpdate};
use yrs::sync::time::SystemClock;
use yrs::updates::{decoder::Decode, encoder::Encode};
               // <─ keep using the wrapper that the rest of yffi expects!

#[repr(C)]
pub struct YAwarenessw {
    inner: RawAwareness,
}

// ───────────────────────────────────────────────────────────────────────────────
// 1) constructor
#[no_mangle]
pub extern "C" fn y_awareness_new(doc: *mut Doc) -> *mut YAwarenessw {
    assert!(!doc.is_null());
    let raw_doc = unsafe { &*doc }.clone();
    let inner   = RawAwareness::with_clock(raw_doc, SystemClock);
    Box::into_raw(Box::new(YAwarenessw { inner }))
}

// 2) free
#[no_mangle]
pub unsafe extern "C" fn y_awareness_destroy(ptr: *mut YAwarenessw) {
    if !ptr.is_null() { drop(Box::from_raw(ptr)); }
}

// 3) encode update
#[no_mangle]
pub unsafe extern "C" fn y_awareness_encode_update(
    ptr: *mut YAwarenessw,
    client_ids: *const u64,
    len: u32,
    out_len: *mut u32,
) -> *mut u8 {
    assert!(!ptr.is_null());
    let aw = &(*ptr).inner;

    let upd = if client_ids.is_null() {
        aw.update().unwrap()
    } else {
        let ids = slice::from_raw_parts(client_ids, len as usize).to_vec();
        aw.update_with_clients(ids).unwrap()
    };

    let bytes = upd.encode_v1().into_boxed_slice();
    *out_len = bytes.len() as u32;
    Box::into_raw(bytes) as *mut u8           // caller frees via ybinary_destroy
}

// 4) apply update
#[no_mangle]
pub unsafe extern "C" fn y_awareness_apply_update(
    ptr: *mut YAwarenessw,
    buf: *const u8,
    len: u32,
) {
    assert!(!ptr.is_null());
    let slice = slice::from_raw_parts(buf, len as usize);
    if let Ok(update) = AwarenessUpdate::decode_v1(slice) {
        let _ = (*ptr).inner.apply_update(update);
    }
}

#[no_mangle]
pub unsafe extern "C" fn y_awareness_set_local_state(
    ptr: *mut YAwarenessw,
    json: *const c_char,               // NULL → clean
) {
    use std::ffi::CStr;
    let aw = &mut (*ptr).inner;

    if json.is_null() {
        aw.clean_local_state();
    } else {
        // CStr  ->  &str
        let s = CStr::from_ptr(json).to_str().unwrap();
        aw.set_local_state_raw(s);     // ← remove the `.into()`
    }
}

#[no_mangle]                                   // getLocalState() -> char* | NULL
pub unsafe extern "C" fn y_awareness_get_local_state(ptr: *mut YAwarenessw)
                                                     -> *mut c_char {
    match (*ptr).inner.local_state_raw() {
        None => std::ptr::null_mut(),
        Some(j) => CString::new(j.as_ref()).unwrap().into_raw(),
    }
}

/* ------------------------------------------------------------------ */
/* 5) remove states -- y_awareness_remove_states                      */
/* ------------------------------------------------------------------ */
#[no_mangle]
pub unsafe extern "C" fn y_awareness_remove_states(
    ptr: *mut YAwarenessw,
    client_ids: *const u64,
    len: u32,
) {
    if ptr.is_null() || client_ids.is_null() { return; }
    let aw = &mut (*ptr).inner;
    for &id in std::slice::from_raw_parts(client_ids, len as usize) {
        aw.remove_state(id);
    }
}

/* ------------------------------------------------------------------ */
/* 6) get full states as JSON string -- y_awareness_get_states         */
/* ------------------------------------------------------------------ */
#[no_mangle]
pub unsafe extern "C" fn y_awareness_get_states(ptr: *mut YAwarenessw)
                                                -> *mut c_char         // caller must free with ystring_destroy
{
    if ptr.is_null() { return std::ptr::null_mut(); }

    let mut out = serde_json::Map::new();
    for (client, st) in (*ptr).inner.iter() {
        if let Some(data) = &st.data {
            match serde_json::from_str::<serde_json::Value>(data) {
                Ok(v) => { out.insert(client.to_string(), v); }
                Err(_) => { /* ignore malformed */ }
            }
        }
    }
    CString::new(serde_json::Value::Object(out).to_string())
        .unwrap()
        .into_raw()
}
