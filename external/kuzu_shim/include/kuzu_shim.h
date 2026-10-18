/*
 * kuzu_shim.h — stable C ABI surface of the embedded Kùzu graph database.
 *
 * These are the exact symbols build.rs verifies with llvm-nm after the CMake
 * step. The Rust FFI layer (src/graph/mod.rs) declares matching extern "C"
 * signatures. Do not rename without updating BOTH the verifier list in
 * build.rs and the extern block in Rust.
 */
#ifndef KUZU_SHIM_H
#define KUZU_SHIM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct kuzu_database kuzu_database;
typedef struct kuzu_connection kuzu_connection;

/* Core lifecycle API — verified by build.rs::verify_exported_symbols(). */
bool kuzu_database_open(const char *db_path, uint64_t buffer_manager_size,
                        kuzu_database **out_database);
bool kuzu_database_close(kuzu_database *database);

bool kuzu_connection_create(kuzu_database *database, kuzu_connection **out_connection);
bool kuzu_connection_destroy(kuzu_connection *connection);

/* Execute a Cypher statement. Returns true on success; on failure the
 * out_error_msg is heap-allocated UTF-8 that the caller frees with
 * kuzu_free_string(). */
bool kuzu_connection_query(kuzu_connection *connection, const char *query,
                           char **out_error_msg);

uint64_t kuzu_connection_num_query_results(kuzu_connection *connection);

void kuzu_free_string(char *s);

/* Version probe so the Rust side can assert engine compatibility at startup. */
const char *kuzu_version(void);

#ifdef __cplusplus
}
#endif

#endif /* KUZU_SHIM_H */
