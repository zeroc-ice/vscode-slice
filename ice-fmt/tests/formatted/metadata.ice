[["cpp:include:list"]]
[["java:package:com.example", "js:module:demo"]]

["cs:namespace:Example"]
module Demo
{
    // The metadata of a definition with a body goes on the line above it.
    ["deprecated"]
    struct Old { int x; }

    ["deprecated"]
    enum Older
    {
        First
    }

    // The metadata of any other declaration stays on its line or on the line above, as written.
    ["deprecated"] sequence<Old> OldSeq;
    ["deprecated"]
    sequence<Older> OlderSeq;

    // The directives go one per line when they don't fit on one line, and what they lead starts the next line.
    ["cpp:type:std::map<std::string, std::vector<std::string>>",
        "java:type:java.util.TreeMap<String, java.util.List<String>>",
        "cs:generic:SortedDictionary"]
    dictionary<string, Ice::StringSeq> Index;

    // Metadata inside a declaration stays on the line of what it applies to.
    sequence<["cpp:type:std::list<std::int32_t>"] int> Numbers;

    // The metadata moves to the line above a declaration that doesn't fit after it.
    ["cpp:type:std::deque<std::wstring>", "java:type:java.util.LinkedList<String>"]
    sequence<["cpp:type:wstring"] string> Lines;

    // Then the types of a sequence or dictionary go one per line.
    dictionary<
        ["cpp:type:wstring"] string,
        ["cpp:type:std::unordered_map<std::string, std::deque<std::int32_t>>"] Numbers> NumbersByName;

    ["cpp:type:std::map<std::wstring, NumbersByName>", "java:type:java.util.TreeMap<String, java.util.Map>"]
    dictionary<
        ["cpp:type:wstring"] string,
        ["cpp:type:std::unordered_map<std::string, Numbers>"] NumbersByName> Everything;

    class Job
    {
        ["cpp:type:wstring"] string owner;
        ["deprecated:use owner"]
        string user;
        ["java:type:java.util.LinkedHashMap<String, String[]>:java.util.LinkedHashMap<String, String[]>"]
        optional(1) Index history;
    }

    interface Printer
    {
        void print(["cpp:array"] Ice::ByteSeq bytes, ["cpp:type:wstring"] string name);

        void printAll(
            ["java:type:java.util.LinkedHashMap<String, String[]>:java.util.LinkedHashMap<String, String[]>"]
            out Index pages,
            int copies);
    }
}
