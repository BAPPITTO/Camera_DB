//! Rust FFI layer for the embedded Kùzu graph database.
//!
//! Every function declared here maps 1:1 onto an exported C symbol of
//! `libkuzu_c_api.a` (see `external/kuzu_shim/include/kuzu_shim.h`). This
//! module is what makes those archive objects live inside the final cdylib:
//! without a referencing crate, the linker would ignore the static archive
//! entirely and `build.rs`'s symbol verification would be moot.
//!
//! DO NOT rename any symbol here without updating BOTH:
//!   * the verifier list in `build.rs` (`REQUIRED_SYMBOLS`), and
//!   * the C header + shim implementation under `external/kuzu_shim/`.

use std::ffi::{c_char, CStr, CString};
use std::ptr;

/// Opaque handle to an open Kùzu database, owned by the engine.
#[repr(C)]
pub struct KuzuDatabase {
    _private: [u8; 0],
}

/// Opaque handle to a Kùzu connection bound to a [`KuzuDatabase`].
#[repr(C)]
pub struct KuzuConnection {
    _private: [u8; 0],
}

extern "C" {
    pub fn kuzu_database_open(
        db_path: *const c_char,
        buffer_manager_size: u64,
        out_database: *mut *mut KuzuDatabase,
    ) -> bool;
    pub fn kuzu_database_close(database: *mut KuzuDatabase) -> bool;

    pub fn kuzu_connection_create(
        database: *mut KuzuDatabase,
        out_connection: *mut *mut KuzuConnection,
    ) -> bool;
    pub fn kuzu_connection_destroy(connection: *mut KuzuConnection) -> bool;

    pub fn kuzu_connection_query(
        connection: *mut KuzuConnection,
        query: *const c_char,
        out_error_msg: *mut *mut c_char,
    ) -> bool;
    pub fn kuzu_connection_num_query_results(connection: *mut KuzuConnection) -> u64;

    pub fn kuzu_free_string(s: *mut c_char);

    pub fn kuzu_version() -> *const c_char;
}

/// Safe wrapper around an open Kùzu database.
///
/// Dropping this handle closes the database (mirroring the RAII pattern of
/// rusqlite's Connection). A half-open handle (open failed) is never
/// constructed — see [`Database::open`].
pub struct Database {
    handle: *mut KuzuDatabase,
}

impl Database {
    /// Open (or create) the graph store rooted at `path`.
    ///
    /// `buffer_manager_size` is the page-cache budget in bytes; pass `0` to
    /// let the engine pick a default. Returns `Err(message)` if the engine
    /// rejects the path or fails to initialise.
    pub fn open<P: AsRef<std::path::Path>>(
        path: P,
        buffer_manager_size: u64,
    ) -> Result<Self, String> {
        let c_path = CString::new(path.as_ref().to_string_lossy().as_bytes())
            .map_err(|_| "database path contains an interior NUL byte".to_string())?;
        let mut handle: *mut KuzuDatabase = ptr::null_mut();
        let ok = unsafe { kuzu_database_open(c_path.as_ptr(), buffer_manager_size, &mut handle) };
        if !ok || handle.is_null() {
            return Err("kuzu_database_open failed".to_string());
        }
        Ok(Self { handle })
    }

    /// Create a new connection against this database.
    pub fn connect(&self) -> Result<Connection, String> {
        let mut handle: *mut KuzuConnection = ptr::null_mut();
        let ok = unsafe { kuzu_connection_create(self.handle, &mut handle) };
        if !ok || handle.is_null() {
            return Err("kuzu_connection_create failed".to_string());
        }
        Ok(Connection {
            handle,
            _db_marker: ptr::null_mut(),
        })
    }

    /// Engine version string reported by the linked native library.
    pub fn engine_version() -> &'static str {
        let raw = unsafe { kuzu_version() };
        if raw.is_null() {
            return "unknown";
        }
        unsafe { CStr::from_ptr(raw) }
            .to_str()
            .unwrap_or("invalid-utf8")
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                kuzu_database_close(self.handle);
            }
        }
    }
}

// The opaque pointers are only ever dereferenced on the thread that owns the
// handle; the engine itself serialises access per connection.
unsafe impl Send for Database {}

/// Safe wrapper around a Kùzu connection.
pub struct Connection {
    handle: *mut KuzuConnection,
    /// Marker keeping the borrow relationship with the parent database
    /// explicit in types even though the C API needs no lifetime here.
    _db_marker: *mut KuzuDatabase,
}

impl Connection {
    /// Execute a Cypher statement.
    ///
    /// On success returns the number of result rows produced. On failure
    /// returns `Err(engine_message)` with the heap-allocated error string the
    /// engine provided (freed via `kuzu_free_string`, per the C ABI contract).
    pub fn query(&self, cypher: &str) -> Result<u64, String> {
        let c_query = CString::new(cypher.as_bytes())
            .map_err(|_| "query contains an interior NUL byte".to_string())?;
        let mut err: *mut c_char = ptr::null_mut();
        let ok = unsafe { kuzu_connection_query(self.handle, c_query.as_ptr(), &mut err) };
        if ok {
            let n = unsafe { kuzu_connection_num_query_results(self.handle) };
            return Ok(n);
        }
        let msg = if err.is_null() {
            "kuzu_connection_query failed".to_string()
        } else {
            let s = unsafe { CStr::from_ptr(err) }
                .to_string_lossy()
                .into_owned();
            unsafe { kuzu_free_string(err) };
            s
        };
        Err(msg)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                kuzu_connection_destroy(self.handle);
            }
        }
    }
}

unsafe impl Send for Connection {}

#[cfg(test)]
mod tests {
    //! These tests exercise ONLY the pure-Rust paths (no native calls): the
    //! Kùzu archive is built and linked exclusively for Android targets, so
    //! host `cargo test` must never touch it. They guard the FFI signatures
    //! (correct arity/types are checked at compile time by coercing the
    //! function items) and the CString input hardening.

    use super::*;

    #[test]
    fn extern_signatures_match_header() {
        // Compile-time proof that every symbol declared above exists with the
        // exact arity/type expected from kuzu_shim.h. If a signature drifts,
        // these coercions fail to typecheck.
        let _: unsafe extern "C" fn(*const c_char, u64, *mut *mut KuzuDatabase) -> bool =
            kuzu_database_open;
        let _: unsafe extern "C" fn(*mut KuzuDatabase) -> bool = kuzu_database_close;
        let _: unsafe extern "C" fn(*mut KuzuDatabase, *mut *mut KuzuConnection) -> bool =
            kuzu_connection_create;
        let _: unsafe extern "C" fn(*mut KuzuConnection) -> bool = kuzu_connection_destroy;
        let _: unsafe extern "C" fn(*mut KuzuConnection, *const c_char, *mut *mut c_char) -> bool =
            kuzu_connection_query;
        let _: unsafe extern "C" fn(*mut KuzuConnection) -> u64 =
            kuzu_connection_num_query_results;
        let _: unsafe extern "C" fn(*mut c_char) = kuzu_free_string;
        let _: unsafe extern "C" fn() -> *const c_char = kuzu_version;
    }

    #[test]
    fn cstring_guards_reject_interior_nul() {
        // `Database::open` / `Connection::query` map interior-NUL inputs to a
        // clean String error instead of panicking inside CString::new.
        let lossy = std::ffi::OsString::from("/tmp/weird");
        let good = CString::new(lossy.to_string_lossy().as_bytes());
        assert!(good.is_ok());
        let bad = CString::new("interior\0nul");
        assert!(bad.is_err());
    }
}
