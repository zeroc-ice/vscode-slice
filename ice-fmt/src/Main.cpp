// Copyright (c) ZeroC, Inc.

#include "Formatter.h"

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <sstream>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

using namespace std;
using namespace IceFmt;

namespace
{
    // Returns the Slice files in a directory and in the directories below it, in order. A name that starts with a
    // dot is skipped, and so is node_modules, where the Ice package for JavaScript keeps its own Slice files.
    vector<string> sliceFilesIn(const string& directory)
    {
        vector<string> files;
        filesystem::recursive_directory_iterator end;
        for (filesystem::recursive_directory_iterator entry{directory}; entry != end; ++entry)
        {
            string name = entry->path().filename().string();
            if (name.starts_with('.') || name == "node_modules")
            {
                if (entry->is_directory())
                {
                    entry.disable_recursion_pending();
                }
            }
            else if (entry->is_regular_file() && entry->path().extension() == ".ice")
            {
                files.push_back(entry->path().string());
            }
        }
        ranges::sort(files);
        return files;
    }

    void usage(ostream& out, string_view program)
    {
        out << "Usage: " << program << " [options] paths...\n";
        out << "Formats .ice Slice files in place. A directory stands for the .ice files in it and below it.\n"
               "Options:\n"
               "-h, --help               Show this message.\n"
               "-v, --version            Display the version.\n"
               "--check                  Write nothing: list the files that formatting would change, and fail when\n"
               "                         there is one.\n";
    }
}

int
main(int argc, char* argv[])
{
    bool check = false;
    vector<string> paths;
    for (int i = 1; i < argc; ++i)
    {
        string_view argument = argv[i];
        if (argument == "-h" || argument == "--help")
        {
            usage(cout, argv[0]);
            return EXIT_SUCCESS;
        }
        if (argument == "-v" || argument == "--version")
        {
            cout << ICE_FMT_VERSION << endl;
            return EXIT_SUCCESS;
        }

        if (argument == "--check")
        {
            check = true;
        }
        else if (argument.starts_with('-'))
        {
            cerr << argv[0] << ": error: unknown option '" << argument << "'" << endl;
            usage(cerr, argv[0]);
            return EXIT_FAILURE;
        }
        else
        {
            paths.emplace_back(argument);
        }
    }

    if (paths.empty())
    {
        cerr << argv[0] << ": error: no input file" << endl;
        usage(cerr, argv[0]);
        return EXIT_FAILURE;
    }

    int status = EXIT_SUCCESS;
    vector<string> files;
    for (const string& path : paths)
    {
        try
        {
            if (filesystem::is_directory(path))
            {
                ranges::move(sliceFilesIn(path), back_inserter(files));
            }
            else
            {
                files.push_back(path);
            }
        }
        catch (const system_error& error)
        {
            cerr << argv[0] << ": error: " << error.what() << endl;
            status = EXIT_FAILURE;
        }
    }

    for (const string& file : files)
    {
        ifstream in{file, ios::binary};
        if (!in)
        {
            cerr << argv[0] << ": error: cannot open '" << file << "' for reading" << endl;
            status = EXIT_FAILURE;
            continue;
        }
        ostringstream buffer;
        buffer << in.rdbuf();
        string text = buffer.str();

        string formatted;
        try
        {
            formatted = formatSlice(text);
        }
        catch (const SyntaxError& error)
        {
            cerr << file << ':' << error.line << ": " << error.what() << endl;
            status = EXIT_FAILURE;
            continue;
        }

        if (formatted == text)
        {
            continue;
        }
        if (check)
        {
            cout << file << endl;
            status = EXIT_FAILURE;
            continue;
        }

        ofstream out{file, ios::binary};
        out << formatted;
        out.close();
        if (!out)
        {
            cerr << argv[0] << ": error: cannot write '" << file << "'" << endl;
            status = EXIT_FAILURE;
        }
    }
    return status;
}
