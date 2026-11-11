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
target_compile_definitions(ice-fmt PRIVATE ICE_FMT_VERSION="${PROJECT_VERSION}")

if(MSVC)
    target_compile_options(ice-fmt PRIVATE /W4)
    # The executable runs without the Visual C++ redistributable.
    set_target_properties(ice-fmt PROPERTIES MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
else()
    target_compile_options(ice-fmt PRIVATE -Wall -Wextra -Wshadow)
endif()

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    # The executable runs on any distribution, whatever its C and C++ libraries.
    target_link_options(ice-fmt PRIVATE -static)
endif()
