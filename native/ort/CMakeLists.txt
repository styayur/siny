# SPDX-License-Identifier: AGPL-3.0-only
# Original Siny build driver. Upstream ORT retains its own MIT/third-party licences.
cmake_minimum_required(VERSION 3.28)
cmake_policy(SET CMP0091 NEW)
set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
project(siny_ort_runtime LANGUAGES C CXX)
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded" CACHE STRING "Static release CRT" FORCE)
foreach(flag ONNX_USE_MSVC_STATIC_RUNTIME protobuf_MSVC_STATIC_RUNTIME ABSL_MSVC_STATIC_RUNTIME)
  set(${flag} ON CACHE BOOL "" FORCE)
endforeach()
set(onnxruntime_BUILD_SHARED_LIB OFF CACHE BOOL "" FORCE)
set(onnxruntime_BUILD_UNIT_TESTS OFF CACHE BOOL "" FORCE)
add_subdirectory("${ONNXRUNTIME_SOURCE_DIR}/cmake" runtime EXCLUDE_FROM_ALL)
# ORT's public C API is supplied by these static components and their dependencies.
set(archive_targets onnxruntime_session onnxruntime_optimizer onnxruntime_providers
  onnxruntime_util onnxruntime_framework onnxruntime_graph onnxruntime_mlas
  onnxruntime_common onnxruntime_flatbuffers onnx_test_data_proto onnx onnx_proto
  libprotobuf-lite re2 absl_base absl_throw_delegate absl_raw_hash_set absl_hash
  absl_city absl_low_level_hash cpuinfo)
set(archive_inputs)
foreach(component IN LISTS archive_targets)
  list(APPEND archive_inputs "$<TARGET_FILE:${component}>")
endforeach()
set(bundled_archive "${CMAKE_CURRENT_BINARY_DIR}/onnxruntime.lib")
add_custom_command(OUTPUT "${bundled_archive}"
  COMMAND "${CMAKE_AR}" /NOLOGO "/OUT:${bundled_archive}" ${archive_inputs}
  DEPENDS ${archive_targets} COMMAND_EXPAND_LISTS VERBATIM)
add_custom_target(siny_runtime ALL DEPENDS "${bundled_archive}")
install(FILES "${bundled_archive}" DESTINATION lib)
install(DIRECTORY "${ONNXRUNTIME_SOURCE_DIR}/include/onnxruntime/core/session/"
  DESTINATION include FILES_MATCHING PATTERN "*.h")
