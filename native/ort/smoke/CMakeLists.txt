# SPDX-License-Identifier: AGPL-3.0-only
cmake_minimum_required(VERSION 3.28)
cmake_policy(SET CMP0091 NEW)
project(siny_ort_smoke LANGUAGES CXX)
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")
add_executable(siny_ort_smoke smoke.cc)
target_compile_features(siny_ort_smoke PRIVATE cxx_std_17)
target_include_directories(siny_ort_smoke PRIVATE "${ONNXRUNTIME_INCLUDE_DIR}")
target_link_libraries(siny_ort_smoke PRIVATE "${ONNXRUNTIME_LIB_DIR}/onnxruntime.lib"
  advapi32 dbghelp ws2_32 bcrypt shlwapi pathcch)
enable_testing()
add_test(NAME ort-c-api COMMAND siny_ort_smoke)
