#pragma once

#include <tyt_tool/fw.hpp>
#include <fstream>
#include <cstring>
#include <sstream>
#include <iomanip>

namespace tyt_tool::fw
{
    class TYTFW : public FW
    {
    public:
        TYTFW(const char cMagic[2])
            : startMagic("OutSecurityBin\0"),
              counterMagic({cMagic[0], cMagic[1]}),
              endMagic({'O', 'u', 't', 'p', 'u', 't', 'B', 'i', 'n', 'D', 'a', 't', 'a', 'E', 'n', 'd'})
        {
        }

        auto Read(const std::string file) -> void
        {
            const auto HeaderSize = 0x100;

            auto i = std::ifstream(file, std::ios_base::binary);
            if (i.is_open())
            {
                char magic_1[16];
                i.read(magic_1, 16);

                if (strcmp(magic_1, startMagic) != 0)
                {
                    throw std::runtime_error("Not a valid firmware file");
                }

                radio.resize(16);
                i.read(radio.data(), 16);
                radio.resize(strlen(radio.c_str()));

                i.read((char *)&n1, 4);
                i.read((char *)&n2, 4);
                i.read((char *)&n3, 4);
                i.read((char *)&n4, 4);

                uint8_t magic_middle[76];
                i.read((char *)magic_middle, 76);
                if (magic_middle[0] != counterMagic[0] || magic_middle[1] != counterMagic[1])
                {
                    throw std::runtime_error("Invalid counter magic");
                }

                uint32_t nRegions = 0, binarySize = 0;
                i.read((char *)&nRegions, 4);
                if ((nRegions * 8) > 0x80)
                {
                    throw std::runtime_error("Memory region count out of bounds");
                }
                for (auto nMem = 0; nMem < nRegions; nMem++)
                {
                    uint32_t rStart = 0, rLength = 0;
                    i.read((char *)&rStart, 4);
                    i.read((char *)&rLength, 4);
                    memory_ranges.push_back(std::make_pair(rStart, rLength));
                    binarySize += rLength;
                }

                //skip to binary
                i.seekg(HeaderSize);

                data.resize(binarySize);
                i.read((char *)data.data(), data.size());

                //meh ignore footer
            }
            i.close();
        }

        auto Write(const std::string) -> void
        {
        }

        auto ToString() const -> std::string
        {
            std::stringstream out;
            out << "Radio: " << radio << std::endl
                << "Size:  " << std::fixed << std::setprecision(2) << (data.size() / 1000.0) << " KB" << std::endl
                << "Memory Map: " << std::endl;
            auto n = 0;
            for (const auto &m : memory_ranges)
            {
                out << "  " << n++ << ": Start=0x" << std::setfill('0') << std::setw(8) << std::hex << m.first
                    << ", Length=0x" << std::setfill('0') << std::setw(8) << std::hex << m.second
                    << std::endl;
            }
            return out.str();
        }

    private:
        const char startMagic[16];
        const char counterMagic[2];
        const char endMagic[16];

        uint32_t n1, n2, n3, n4;
        std::string radio;
    };
} // namespace tytfw::fw