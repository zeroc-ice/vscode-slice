# Copyright (c) ZeroC, Inc.

cmake_minimum_required(VERSION 3.24)

# The oldest macOS the executable runs on.
set(CMAKE_OSX_DEPLOYMENT_TARGET 11.0 CACHE STRING "")

project(ice-fmt VERSION 0.1.0 LANGUAGES CXX)

add_executable(ice-fmt
    src/Formatter.cpp
    src/Layout.cpp
    src/Main.cpp
    src/SyntaxParser.cpp
    src/SyntaxScanner.cpp)

target_compile_features(ice-fmt PRIVATE cxx_std_20)
set_target_properties(ice-fmt PROPERTIES CXX_EXTENSIONS OFF)
target_compile_definitions(ice-fmt PRIVATE ICE_FMT_VERSION="${PROJECT_VERSION}")

if(MSVC)
    target_compile_options(ice-fmt PRIVATE /W4)
    # The executable runs without the Visual C++ redistributable.
    set_target_properties(ice-fmt PROPERTIES MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
else()
    target_compile_options(ice-fmt PRIVATE -Wall -Wextra -Wshadow -Wconversion -Wsign-conversion -Wpedantic)
endif()

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    # The executable runs on any distribution, whatever its C and C++ libraries.
    target_link_options(ice-fmt PRIVATE -static)
endif()

enable_testing()

set(tests "${CMAKE_CURRENT_SOURCE_DIR}/tests")
set(output "${CMAKE_CURRENT_BINARY_DIR}/tests")
file(MAKE_DIRECTORY "${output}")

# Each case is a file in tests/unformatted, and the file ice-fmt makes of it in tests/formatted.
foreach(case comments crlf definitions metadata operations)
    add_test(NAME format-${case} COMMAND "${CMAKE_COMMAND}"
        -DICE_FMT=$<TARGET_FILE:ice-fmt>
        -DUNFORMATTED=${tests}/unformatted/${case}.ice
        -DFORMATTED=${tests}/formatted/${case}.ice
        -DOUTPUT=${output}/${case}.ice
        -P "${tests}/FormatTest.cmake")

    # --check lists a file that formatting would change, fails, and writes nothing.
    add_test(NAME check-${case} COMMAND "${CMAKE_COMMAND}"
        -DICE_FMT=$<TARGET_FILE:ice-fmt>
        -DUNFORMATTED=${tests}/unformatted/${case}.ice
        -DOUTPUT=${output}/check-${case}.ice
        -P "${tests}/CheckTest.cmake")

    # A formatted file is left as it is.
    add_test(NAME keep-${case} COMMAND ice-fmt --check "${tests}/formatted/${case}.ice")
endforeach()

# A file with the comment "// ice-fmt: ignore-file" is left as it is.
add_test(NAME ignore-file COMMAND ice-fmt --check "${tests}/ignore-file.ice")

# A directory stands for the .ice files below it, except under a name that starts with a dot and under node_modules.
# The directory is made here because the repository ignores node_modules.
set(directory "${output}/directory")
file(WRITE "${directory}/nested/found.ice" "module  Demo {}\n")
file(WRITE "${directory}/nested/skipped.txt" "module  Demo {}\n")
file(WRITE "${directory}/.hidden/skipped.ice" "module  Demo {}\n")
file(WRITE "${directory}/node_modules/skipped.ice" "module  Demo {}\n")
add_test(NAME directory COMMAND ice-fmt --check "${directory}")
set_tests_properties(directory PROPERTIES
    PASS_REGULAR_EXPRESSION "found\\.ice"
    FAIL_REGULAR_EXPRESSION "skipped\\.")
