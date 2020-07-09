#pragma once

#include <exception>
#include <string>

namespace tytfw::dfu {
    class DFUException : public std::exception {
    public:
        DFUException(const std::string& str) 
            : msg(str) { }

        auto what() const noexcept -> const char* {
            return msg.c_str();
        }
    private:
        const std::string msg;
    };
}