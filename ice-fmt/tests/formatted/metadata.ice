[["cpp:include:list"]]
[["java:package:com.example", "js:module:demo"]]

["cs:namespace:Example"] module Demo
{
    // Metadata stays on the line of its declaration or on the line above, as written.
    ["deprecated"] struct Old { int x; }

    ["deprecated"]
    struct Older { int x; }

    // The directives go one per line when they don't fit on one line.
    ["cpp:type:std::map<std::string, std::vector<std::string>>",
        "java:type:java.util.TreeMap<String, java.util.List<String>>",
        "cs:generic:SortedDictionary"]
    dictionary<string, Ice::StringSeq> Index;

    class Job
    {
        ["cpp:type:wstring"] string owner;
        ["deprecated:use owner"]
        string user;
    }

    interface Printer
    {
        void print(["cpp:array"] Ice::ByteSeq bytes, ["cpp:type:wstring"] string name);
    }
}
