#pragma once

#include <tyt_tool/dfu.hpp>

namespace tyt_tool::dfu
{
    enum class TYTCommand : uint8_t {
        ProgrammingMode = 0x01,
        SetRTC = 0x02,
        Reboot = 0x05,
        ReadRegister = 0xa2
    };

    constexpr auto TYTRegisterSize = 1024;
    
    enum class TYTRegister : uint8_t {
        RadioInfo = 0x01, //Radio Model(16 bytes) + 16 bytes of something else
        R_02 = 0x02, //unknown (4 bytes)
        R_03 = 0x03, //unknown (24 bytes)
        R_04 = 0x04, //unknown (8 bytes)
        R_07 = 0x07, //unknown (16 bytes)
        RTC = 0x08, //Real time clock (7 bytes)
    };

    class TYT : public DFU
    {
    public:
        TYT(const uint16_t vid, const uint16_t pid, const uint16_t idx = 0)
            : DFU(vid, pid, idx)
        {
        }
        auto IdentifyDevice() const -> std::string;
        auto ReadRegister(const TYTRegister& reg) const -> std::vector<uint8_t>;
    protected:
        auto InitDownload() const -> void;
        auto InitUpload() const -> void;
    };
} // namespace tyt_tool::dfu