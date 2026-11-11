// Copyright (c) ZeroC, Inc.
#pragma once

  #include "Ice/Identity.ice"


/// A doc comment keeps its text,   spaces included.
module Demo
{
            // A comment on its own line takes the indentation of the code.
    enum Color
    {
        red,    // A comment that follows code keeps its column.
        green,  // green
        blue    // blue
    }

      /* A block comment keeps the indentation
         of its lines relative to the first one.

           Its blank lines stay. */
    struct Point { int x; int y; }

#ifdef WITH_3D
    struct Point3D { int x; int y; int z; }
#endif

    interface Printer
    {
        void print(string document /* inline */, int copies); // A comment after an operation.

        /// @param user The user.
        void pause(
            string user, // why
            bool notify);

        // A comment before the closing brace.
    }

    struct Pair { int first; int second; }; // The comment moves over when the semicolon goes.
}
// The end.
