use crate::dyld::{export_c_func, FunctionExports, HostDylib};
use crate::mem::{ConstPtr, MutPtr};
use crate::Environment;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;

const SQLITE_OK: u32 = 0;
const SQLITE_ERROR: u32 = 1;
const SQLITE_ROW: u32 = 100;
const SQLITE_DONE: u32 = 101;

// We need a lifetime-safe way to manage statements. To keep things clean in a static map,
// we will safely box the statements or track them dynamically. 
pub struct StatementWrapper {
    // We store the SQL text and connection handle so we can lazily prepare 
    // or execute steps without breaking rusqlite's strict lifetime bounds.
    pub conn_handle: u32,
    pub sql: String,
}

lazy_static::lazy_static! {
    static ref SQLITE_CONNECTIONS: Mutex<HashMap<u32, Connection>> = Mutex::new(HashMap::new());
    static ref SQLITE_STATEMENTS: Mutex<HashMap<u32, StatementWrapper>> = Mutex::new(HashMap::new());
    static ref NEXT_HANDLE: Mutex<u32> = Mutex::new(0x8000_0000);
}

// Helper to safely extract a null-terminated string out of guest memory
fn read_guest_string(env: &Environment, ptr: u32) -> String {
    if ptr == 0 { return String::new(); }
    let mut bytes = Vec::new();
    let mut current_addr = ptr;
    loop {
        let p: ConstPtr<u8> = ConstPtr::from_bits(current_addr);
        let byte: u8 = env.mem.read(p);
        if byte == 0 { break; }
        bytes.push(byte);
        current_addr += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

// int sqlite3_open(const char *filename, sqlite3 **ppDb);
pub fn sqlite3_open(env: &mut Environment, filename_ptr: u32, pp_db: u32) -> u32 {
    let filename = read_guest_string(env, filename_ptr);
    let safe_name = filename.replace("/", "_");
    let path = format!(
        "/storage/emulated/0/Android/data/org.touchhle.android.unofficial/files/touchHLE_apps/{}",
        safe_name
    );

    match Connection::open(&path) {
        Ok(conn) => {
            let mut handles = SQLITE_CONNECTIONS.lock().unwrap();
            let mut next_id = NEXT_HANDLE.lock().unwrap();

            let handle = *next_id;
            *next_id += 4;
            handles.insert(handle, conn);

            let pp_db_ptr: MutPtr<u32> = MutPtr::from_bits(pp_db);
            env.mem.write(pp_db_ptr, handle);

            SQLITE_OK
        }
        Err(e) => {
            println!("libsqlite3: Failed to open DB: {}", e);
            SQLITE_ERROR
        }
    }
}

pub fn sqlite3_open_v2(env: &mut Environment, filename_ptr: u32, pp_db: u32) -> u32 {
    // Reuses the same robust open logic
    sqlite3_open(env, filename_ptr, pp_db)
}

// int sqlite3_close(sqlite3 *pDb);
pub fn sqlite3_close(_env: &mut Environment, p_db: u32) -> u32 {
    let mut handles = SQLITE_CONNECTIONS.lock().unwrap();
    if handles.remove(&p_db).is_some() {
        SQLITE_OK
    } else {
        SQLITE_ERROR
    }
}

pub fn sqlite3_close_v2(_env: &mut Environment, p_db: u32) -> u32 {
    let mut handles = SQLITE_CONNECTIONS.lock().unwrap();
    if handles.remove(&p_db).is_some() {
        SQLITE_OK
    } else {
        SQLITE_ERROR
    }
}

// int sqlite3_prepare_v2(sqlite3 *db, const char *zSql, int nByte, sqlite3_stmt **ppStmt, const char **pzTail);
pub fn sqlite3_prepare_v2(env: &mut Environment, db_handle: u32, z_sql_ptr: u32, _n_byte: i32, pp_stmt_ptr: u32, pz_tail_ptr: u32) -> u32 {
    let sql = read_guest_string(env, z_sql_ptr);
    
    let mut stmt_handles = SQLITE_STATEMENTS.lock().unwrap();
    let mut next_id = NEXT_HANDLE.lock().unwrap();
    
    let stmt_handle = *next_id;
    *next_id += 4;
    
    // Store the wrapper details to simulate the statement lifecycle
    stmt_handles.insert(stmt_handle, StatementWrapper {
        conn_handle: db_handle,
        sql,
    });
    
    // Write statement handle out to the pointer address the game provided
    let pp_stmt: MutPtr<u32> = MutPtr::from_bits(pp_stmt_ptr);
    env.mem.write(pp_stmt, stmt_handle);
    
    // Handle pzTail if the app requested it (null termination pointer offset fallback)
    if pz_tail_ptr != 0 {
        let pz_tail: MutPtr<u32> = MutPtr::from_bits(pz_tail_ptr);
        env.mem.write(pz_tail, 0); // Stubbable trailing pointer
    }
    
    SQLITE_OK
}

// int sqlite3_step(sqlite3_stmt *pStmt);
pub fn sqlite3_step(_env: &mut Environment, stmt_handle: u32) -> u32 {
    let stmt_map = SQLITE_STATEMENTS.lock().unwrap();
    
    if let Some(stmt_wrapper) = stmt_map.get(&stmt_handle) {
        let conns = SQLITE_CONNECTIONS.lock().unwrap();
        if let Some(conn) = conns.get(&stmt_wrapper.conn_handle) {
            // Because older iOS analytics packages use basic queries like CREATE TABLE, 
            // we can directly route statement execution down through native SQLite.
            match conn.execute(&stmt_wrapper.sql, []) {
                Ok(_) => SQLITE_DONE,
                Err(e) => {
                    println!("libsqlite3: execution error for query ({}): {}", stmt_wrapper.sql, e);
                    SQLITE_ERROR
                }
            }
        } else {
            SQLITE_ERROR
        }
    } else {
        SQLITE_ERROR
    }
}

// int sqlite3_reset(sqlite3_stmt *pStmt);
pub fn sqlite3_reset(_env: &mut Environment, _stmt_handle: u32) -> u32 {
    // Simply return OK to keep execution sequences rolling cleanly 
    SQLITE_OK
}

// int sqlite3_finalize(sqlite3_stmt *pStmt);
pub fn sqlite3_finalize(_env: &mut Environment, stmt_handle: u32) -> u32 {
    let mut stmt_map = SQLITE_STATEMENTS.lock().unwrap();
    stmt_map.remove(&stmt_handle);
    SQLITE_OK
}

// int sqlite3_bind_parameter_count(sqlite3_stmt *pStmt);
pub fn sqlite3_bind_parameter_count(_env: &mut Environment, _stmt_handle: u32) -> i32 {
    0 // Simplified placeholder stub to bypass binding verification checks
}

// sqlite3_int64 sqlite3_last_insert_rowid(sqlite3 *pDb);
pub fn sqlite3_last_insert_rowid(_env: &mut Environment, db_handle: u32) -> u64 {
    let conns = SQLITE_CONNECTIONS.lock().unwrap();
    if let Some(conn) = conns.get(&db_handle) {
        conn.last_insert_rowid() as u64
    } else {
        0
    }
}

// const char *sqlite3_errmsg(sqlite3 *pDb);
pub fn sqlite3_errmsg(_env: &mut Environment, _db_handle: u32) -> u32 {
    0 // Return a null pointer for message handling to avoid triggering analytical panic routes
}

// void sqlite3_free(void *ptr);
pub fn sqlite3_free(env: &mut Environment, ptr: u32) {
    if ptr == 0 {
        return;
    }
    let mem_ptr = crate::mem::MutVoidPtr::from_bits(ptr);
    env.mem.free(mem_ptr);
}

// Register all functions so the dynamic linker maps them correctly
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sqlite3_open(_, _)),
    export_c_func!(sqlite3_close(_)),
    export_c_func!(sqlite3_open_v2(_, _)),
    export_c_func!(sqlite3_close_v2(_)),
    export_c_func!(sqlite3_prepare_v2(_, _, _, _, _)),
    export_c_func!(sqlite3_step(_)),
    export_c_func!(sqlite3_reset(_)),
    export_c_func!(sqlite3_finalize(_)),
    export_c_func!(sqlite3_bind_parameter_count(_)),
    export_c_func!(sqlite3_last_insert_rowid(_)),
    export_c_func!(sqlite3_errmsg(_)),
    export_c_func!(sqlite3_free(_)),
];

pub const DYLIB: HostDylib = HostDylib {
    path: "/usr/lib/libsqlite3.dylib",
    aliases: &["/usr/lib/libsqlite3.0.dylib"],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[FUNCTIONS],
};
