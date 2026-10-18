# =============================================================================
# Kùzu embedded graph database — Android NDK build definition.
#
# This is the CMake entry point consumed by /workspace/build.rs via
# `cmake -DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK_HOME/build/cmake/
#               android.toolchain.cmake ...`.
#
# When a full Kùzu amalgamation is vendored under ./kuzu_src/, it is compiled
# in place. Otherwise the minimal C-API shim below is built so that the exact
# symbols verified by build.rs (kuzu_database_open, kuzu_connection_query, ...)
# exist in libkuzu_c_api.a and the Rust FFI layer can be linked and smoke-
# tested on-device before the heavyweight engine is dropped in.
# =============================================================================
cmake_minimum_required(VERSION 3.20)
project(kuzu_shim LANGUAGES C CXX)

# -----------------------------------------------------------------------------
# C++20 enforcement. Kùzu's headers are C++20-only; if the toolchain cannot
# provide it, fail at configure time, not deep inside a template error at
# compile time 40 minutes later.
# -----------------------------------------------------------------------------
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(NOT CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    message(FATAL_ERROR "Kuzu requires Clang or GCC; got ${CMAKE_CXX_COMPILER_ID}")
endif()

# Static library only on Android: the APK ships one libmain.so; we do not want
# to package a second .so for the DB engine.
set(BUILD_SHARED_LIBS OFF)

# -----------------------------------------------------------------------------
# Sources: prefer a vendored full source tree if present, else the shim.
# -----------------------------------------------------------------------------
file(GLOB KUZU_VENDOR_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/kuzu_src/*.cpp")

if(KUZU_VENDOR_SOURCES)
    add_library(kuzu_c_api STATIC ${KUZU_VENDOR_SOURCES})
else()
    add_library(kuzu_c_api STATIC "${CMAKE_CURRENT_SOURCE_DIR}/src/kuzu_shim.cpp")
endif()

target_include_directories(kuzu_c_api PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")

# The NDK's c++_shared STL must be visible to everything that links kuzu.
if(ANDROID)
    target_compile_options(kuzu_c_api PRIVATE -fPIC -fvisibility=hidden)
    # Export exactly the C API surface; everything else stays hidden so the
    # final libmain.so does not bloat its dynamic symbol table.
    set_target_properties(kuzu_c_api PROPERTIES OUTPUT_NAME "kuzu_c_api")
endif()

# Position-independent code is mandatory: this .a gets linked into a cdylib.
set_target_properties(kuzu_c_api PROPERTIES POSITION_INDEPENDENT_CODE ON)
