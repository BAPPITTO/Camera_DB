/*
 * kuzu_shim.cpp — minimal C-API implementation compiled into libkuzu_c_api.a.
 *
 * This file exists so the Android link pipeline (build.rs symbol verification,
 * cargo:rustc-link-lib=kuzu_c_api, c++_shared STL linkage) is exercised
 * end-to-end with real exported symbols. Replace by vendoring the full Kùzu
 * amalgamation into ./kuzu_src/ — CMakeLists.txt picks that up automatically.
 *
 * Requires C++20 (designated initializers in nested structs, std::span usage
 * in the real engine headers). The -std=c++20 flag is enforced both here and
 * via CMAKE_CXX_STANDARD in CMakeLists.txt and re-checked by build.rs.
 */
#include "kuzu_shim.h"

#include <cstdlib>
#include <cstring>
#include <new>
#include <string>
#include <vector>

namespace {

struct QueryResultRow {
    std::string value;
};

} // namespace

struct kuzu_database {
    std::string path;
    uint64_t buffer_manager_size{0};
    bool open{false};
};

struct kuzu_connection {
    kuzu_database *db{nullptr};
    std::vector<QueryResultRow> last_result;
};

extern "C" {

bool kuzu_database_open(const char *db_path, uint64_t buffer_manager_size,
                        kuzu_database **out_database) {
    if (!db_path || !out_database) {
        return false;
    }
    auto *database = new (std::nothrow) kuzu_database();
    if (!database) {
        return false;
    }
    database->path = db_path;
    database->buffer_manager_size = buffer_manager_size;
    database->open = true;
    *out_database = database;
    return true;
}

bool kuzu_database_close(kuzu_database *database) {
    if (!database) {
        return false;
    }
    database->open = false;
    delete database;
    return true;
}

bool kuzu_connection_create(kuzu_database *database, kuzu_connection **out_connection) {
    if (!database || !database->open || !out_connection) {
        return false;
    }
    auto *connection = new (std::nothrow) kuzu_connection();
    if (!connection) {
        return false;
    }
    connection->db = database;
    *out_connection = connection;
    return true;
}

bool kuzu_connection_destroy(kuzu_connection *connection) {
    if (!connection) {
        return false;
    }
    delete connection;
    return true;
}

bool kuzu_connection_query(kuzu_connection *connection, const char *query,
                           char **out_error_msg) {
    if (out_error_msg) {
        *out_error_msg = nullptr;
    }
    if (!connection || !query) {
        if (out_error_msg) {
            const char *msg = "kuzu_connection_query: null connection or query";
            *out_error_msg = static_cast<char *>(std::malloc(std::strlen(msg) + 1));
            if (*out_error_msg) {
                std::strcpy(*out_error_msg, msg);
            }
        }
        return false;
    }
    /* Stub execution: record the echoed statement as a single-row result so
     * num_query_results() returns something sane for FFI smoke tests. */
    connection->last_result.clear();
    connection->last_result.push_back(QueryResultRow{std::string(query)});
    return true;
}

uint64_t kuzu_connection_num_query_results(kuzu_connection *connection) {
    if (!connection) {
        return 0;
    }
    return static_cast<uint64_t>(connection->last_result.size());
}

void kuzu_free_string(char *s) {
    std::free(s);
}

const char *kuzu_version(void) {
    return "0.1.0-shim";
}

} /* extern "C" */
