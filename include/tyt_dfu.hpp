#pragma once

#include <dfu.hpp>

namespace tytfw::dfu
{
    class TYTDFU : public DFU
    {
    public:
        TYTDFU(const uint16_t vid, const uint16_t pid, const uint16_t idx = 0)
            : DFU(vid, pid, idx)
        {
        }
        auto IdentifyDevice() const -> std::string;
        auto GetState() const -> DFUState;
        auto GetStatus() const -> const DFUStatusReport;
        auto Abort() const -> void;
        auto Detach() const -> void;
        auto EnterDFUMode() const -> void;
    protected:
        auto WaitForIdle() const -> void;
    };
} // namespace tytfw::dfu