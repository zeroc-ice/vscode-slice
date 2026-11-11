module Demo
{
    // A definition written on one line stays on one line. A blank line separates a definition with a body from the
    // definitions around it.
    enum Color { Red, Green, Blue }

    struct Point { int x; int y; }

    // A definition written on several lines has one member per line.
    enum Shape
    {
        Circle,
        Square,
        Triangle
    }

    struct Size
    {
        int width;
        int height;
    }

    exception PrinterException
    {
        string reason;
    }

    // An empty body written as {} follows its definition. Braces written on two lines stay there.
    class Empty {}

    class Spaced {}

    class OwnLine {}

    class Split
    {
    }

    // The semicolon after a closing brace goes. The others stay.
    class Forward;

    interface Named {}

    struct Pair { int first; int second; }

    // An extends clause moves to its own line when it doesn't fit.
    interface Scanner extends Device
    {
        void scan();
    }

    interface MultiFunctionPrinter
        extends Printer, Scanner, Copier, FaxMachineWithAVeryLongNameThatFillsTheLine, Stapler
    {
        void run();
    }

    // One space or none separates two tokens.
    sequence<string> Names;
    dictionary<string, int> Counts;
    const int MaxCopies = 10;

    class Job
    {
        Printer* printer;
        optional(1) string owner;
        int copies = 1;
    }

    module Nested
    {
        enum Mode { Draft, Final }
    }
}
