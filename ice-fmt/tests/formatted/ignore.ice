// Copyright (c) ZeroC, Inc.

module Demo
{
    // Formatting leaves the definitions and members between these two comments as written.
    // ice-fmt: off
    const int    One   = 1;
    const int    Eleven = 11;
    // ice-fmt: on
    const int Twelve = 12;

    interface Printer
    {
        // ice-fmt: ignore
        void print(string  document,
                   int     copies);

        void stop(string user, bool notify);
    }

    enum Color
    {
        // ice-fmt: off

        Red     = 1,
        Green   = 2,
        // Without the comment that turns formatting on, it stays off to the end of the body.
        Blue    = 3
    }
}
