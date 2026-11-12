module Demo
{
    interface Printer
    {
        // An operation that fits on one line goes on one line.
        ["cpp:const"] idempotent string status() throws PrinterException;

        void resume(string user, bool notify);

        // A line of 120 columns fits.
        idempotent Ice::StringSeq listQueuedOnTray(string user, Color color, bool doubleSided, optional(1) string tray);

        // Metadata written on the line above stays there.
        ["amd"]
        void pause(string user);

        // The metadata moves to the line above.
        ["amd"]
        idempotent Ice::StringSeq listQueuedDocuments(string user, Color color, bool doubleSided, optional(1) int max);

        // Then the throws clause moves to its own line.
        void cancel(string user, Ice::StringSeq documents, bool notifyOwner, out Ice::StringSeq cancelled)
            throws PrinterException;

        ["amd"]
        void cancelAll(string user, bool notifyOwners, out Ice::StringSeq cancelled, out int remaining)
            throws PrinterException;

        // Then the parameters go one per line.
        void print(
            string document,
            Color color,
            int copies,
            bool doubleSided,
            optional(1) string tray,
            out int pages,
            out optional(2) string warning)
            throws PrinterException;

        ["amd", "cpp:const"]
        idempotent Ice::StringSeq search(
            ["cpp:array"] Ice::StringSeq terms,
            Color color,
            bool doubleSided,
            optional(1) int max,
            out bool truncated)
            throws PrinterException;

        // Metadata whose directives go one per line puts the operation on the next line.
        ["amd",
            "cpp:const",
            "swift:attribute:@available(*, deprecated, message: \"use search\")",
            "cs:attribute:System.Obsolete(\"use search\")"]
        Ice::StringSeq find(string term);

        optional(1) string describe(
            string document,
            Color color,
            int copies,
            bool doubleSided,
            optional(2) string tray,
            out int pages);

        // The names of a throws clause go one per line when they don't fit on its line.
        void shutdown()
            throws PrinterException,
                QueueException,
                AVeryLongExceptionNameThatFillsTheLine,
                AnotherVeryLongExceptionNameThatFillsTheLine;
    }
}
