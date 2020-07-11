#pragma once

#include <string>
#include <vector>

namespace tyt_tool::fw
{
    class FW
    {
    public:
        virtual auto Read(const std::string) -> void = 0;
        virtual auto Write(const std::string) -> void = 0;
    protected:
        std::vector<uint8_t> data;
        std::vector<std::pair<uint32_t, uint32_t>> memory_ranges;
    };
} // namespace tyt_tool::fw