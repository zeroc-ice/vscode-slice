// Copyright (c) ZeroC, Inc.

// Ice 3.7 syntax: local definitions, and classes with operations that implement interfaces.
module Demo
{
    local interface Logger;
    local interface Logger { void print(string message); }

    ["cpp:type:std::list<std::string>"]
    local sequence<string> StringList;

    local class EndpointInfo extends Info implements Logger, Printer
    {
        optional(1) int timeout;
        short type();
        idempotent bool datagram();
    }

    // Ice 3.8 doesn't reserve these names.
    struct Local { string local; string implements; }
}
