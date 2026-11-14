// Copyright (c) ZeroC, Inc.

#include "Formatter.h"
#include "SyntaxError.h"

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#ifdef _WIN32
#    define NOMINMAX
#    include <windows.h>
#endif

using namespace std;
using namespace IceFmt;

namespace
{
    // Returns a path as text for the console. On Windows, the text is in the code page of the console, as the messages
    // of the Slice compilers are: the code page of the process doesn't have every character a file name can hold.
    string consoleText(const filesystem::path& path)
    {
#ifdef _WIN32
        const wstring& wide = path.native();
        auto wideSize = static_cast<int>(wide.size());
        UINT codePage = GetConsoleOutputCP();
        int size = WideCharToMultiByte(codePage, 0, wide.data(), wideSize, nullptr, 0, nullptr, nullptr);
        string text(static_cast<size_t>(size), '\0');
        WideCharToMultiByte(codePage, 0, wide.data(), wideSize, text.data(), size, nullptr, nullptr);
        return text;
#else
        return path.string();
#endif
    }

    // Returns the .ice files in a directory, recursively, in order. It skips the files and directories whose name
    // starts with a dot, and node_modules directories, where npm installs the Slice files of the Ice for JavaScript
    // package.
    vector<filesystem::path> sliceFilesIn(const filesystem::path& directory)
    {
        vector<filesystem::path> files;
        filesystem::recursive_directory_iterator end;
        for (filesystem::recursive_directory_iterator entry{directory}; entry != end; ++entry)
        {
            filesystem::path name = entry->path().filename();
            if (name.native().starts_with('.') || name == "node_modules")
            {
                if (entry->is_directory())
                {
                    entry.disable_recursion_pending();
                }
            }
            else if (entry->is_regular_file() && name.extension() == ".ice")
            {
                files.push_back(entry->path());
            }
        }
        ranges::sort(files);
        return files;
    }

    // Formats a file in place. With 'check', it writes nothing and lists the file when formatting would change it.
    // Returns false when the file can't be read, formatted, or written, and with 'check' when formatting would change
    // it.
    bool formatFile(const filesystem::path& file, bool check, string_view program)
    {
        string name = consoleText(file);
        ifstream in{file, ios::binary};
        if (!in)
        {
            cerr << program << ": error: cannot open '" << name << "' for reading" << endl;
            return false;
        }
        string text{istreambuf_iterator<char>{in}, istreambuf_iterator<char>{}};

        string formatted;
        try
        {
            formatted = formatSlice(text);
        }
        catch (const SyntaxError& error)
        {
            cerr << name << ':' << error.line << ": " << error.what() << endl;
            return false;
        }
        catch (const logic_error& error)
        {
            cerr << name << ": internal error: " << error.what() << endl;
            return false;
        }

        if (formatted == text)
        {
            return true;
        }
        if (check)
        {
            cout << name << endl;
            return false;
        }

        ofstream out{file, ios::binary};
        out << formatted;
        out.close();
        if (!out)
        {
            cerr << program << ": error: cannot write '" << name << "'" << endl;
            return false;
        }
        return true;
    }

    void usage(ostream& out, string_view program)
    {
        out << "Usage: " << program << " [options] paths...\n";
        out << "Formats .ice Slice files in place. Given a directory, formats all its .ice files, recursively.\n"
               "Options:\n"
               "-h, --help               Show this message.\n"
               "-v, --version            Display the version.\n"
               "--check                  Write nothing: list the files that formatting would change, and fail when\n"
               "                         there is one.\n";
    }
}

// On Windows, the arguments are wide strings, as the Slice compilers take them: the code page of the process doesn't
// have every character a file name can hold.
#ifdef _WIN32
int
wmain(int argc, wchar_t* argv[])
#else
int
main(int argc, char* argv[])
#endif
{
    string program = consoleText(argv[0]);
    bool check = false;
    vector<filesystem::path> paths;
    for (int i = 1; i < argc; ++i)
    {
        filesystem::path argument = argv[i];
        if (argument == "-h" || argument == "--help")
        {
            usage(cout, program);
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
        else if (argument.native().starts_with('-'))
        {
            cerr << program << ": error: unknown option '" << consoleText(argument) << "'" << endl;
            usage(cerr, program);
            return EXIT_FAILURE;
        }
        else
        {
            paths.push_back(std::move(argument));
        }
    }

    if (paths.empty())
    {
        cerr << program << ": error: no input file" << endl;
        usage(cerr, program);
        return EXIT_FAILURE;
    }

    int status = EXIT_SUCCESS;
    vector<filesystem::path> files;
    for (const filesystem::path& path : paths)
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
            cerr << program << ": error: " << error.what() << endl;
            status = EXIT_FAILURE;
        }
    }

    for (const filesystem::path& file : files)
    {
        if (!formatFile(file, check, program))
        {
            status = EXIT_FAILURE;
        }
    }
    return status;
}
