# Copyright (c) ZeroC, Inc.

# Runs the executable ICE_FMT with --check on a copy of the file UNFORMATTED, and fails unless it lists the copy, exits
# with status 1, and leaves the copy as it is. OUTPUT is where the copy goes.

file(COPY_FILE "${UNFORMATTED}" "${OUTPUT}")
execute_process(COMMAND "${ICE_FMT}" --check "${OUTPUT}" RESULT_VARIABLE status OUTPUT_VARIABLE listed)
if(NOT status EQUAL 1)
    message(FATAL_ERROR "ice-fmt --check exited with ${status}")
endif()

string(STRIP "${listed}" listed)
if(NOT listed STREQUAL OUTPUT)
    message(FATAL_ERROR "ice-fmt --check listed '${listed}', not ${OUTPUT}")
endif()

execute_process(COMMAND "${CMAKE_COMMAND}" -E compare_files "${OUTPUT}" "${UNFORMATTED}" RESULT_VARIABLE different)
if(different)
    message(FATAL_ERROR "ice-fmt --check changed ${OUTPUT}")
endif()
