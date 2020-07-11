#pragma once

#include <vector>
#include <stdint.h>
#include <iostream>
#include <sstream>

namespace tyt_tool {
    static auto PrintHex(const std::vector<uint8_t>& data) -> void {
        auto c = 1;

        constexpr auto asciiZero = 0x30;
        constexpr auto asciiA = 0x61 - 0x0a;

        constexpr auto wordSize = 4;
        constexpr auto wordCount = 4;

        char aV, bV;
        std::stringstream prnt;
        for(const auto& v : data) {
            auto a = v & 0x0f;
            auto b = v >> 4;
            aV = (a <= 9 ? asciiZero : asciiA) + a;
            bV = (b <= 9 ? asciiZero : asciiA) + b;
            prnt << bV << aV;
            if(c % (wordSize * wordCount) == 0  && c != data.size()) {
                prnt<< std::endl;
            } else if(c % wordSize == 0) {
                prnt << " ";
            }
            c++;
        }

        std::cerr << prnt.str() << std::endl;
    }
}