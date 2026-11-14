// Copyright (c) ZeroC, Inc.
#pragma once

  #include "Ice/Identity.ice"


/// A doc comment keeps its text,   spaces included.  
module Demo
{
            // A comment on its own line takes the indentation of the code.
    enum Color
    {
        Red, // Comments that follow code on consecutive lines line up.
        Green,      // Green
        Blue  // Blue
    }

    struct Limits
    {
        string aVeryLongMemberNameThatPushesTheTrailingCommentFarToTheRightSide;    // short
        int x;      // Comments line up even when that takes a line past 120 columns.
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

    /** A doc comment before code on its line gets the line to itself. */ struct Triple { int first; int second; int third; }

    struct Options
    {
        int timeout; /// A doc comment after code moves above its line, where the Slice compiler reads it.
        int retries; int delay; /** This one documents retries,
                                    the first definition of its line. */
    }

    // The comments before a closing brace have a line each when the body doesn't fit on one line.
    struct Quad { int first; int second; int third; int fourth; /* The first comment is here. */ /* The second comment is here. */ }

    // The blanks that end a line go, even after a backslash.
    /**
     * A tree:
     *   /\  
     *  /  \  
     */
    struct Tree { int depth; }
}
// The end.
