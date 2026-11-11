[["cpp:include:list"]]
[[ "java:package:com.example" ,"js:module:demo" ]]

["cs:namespace:Example"] module Demo
{
    // Metadata stays on the line of its declaration or on the line above, as written.
    ["deprecated"] struct Old { int x; }
    ["deprecated"]
    struct Older { int x; }

    class Job
    {
        [ "cpp:type:wstring" ]string owner;
        ["deprecated:use owner"]
        string user;
    }

    interface Printer
    {
        void print(["cpp:array"]Ice::ByteSeq bytes,["cpp:type:wstring"] string name);
    }
}
