#include <tyt_tool/tyt_dfu.hpp>
#include <tyt_tool/dfu_exception.hpp>

#include <chrono>
#include <thread>
#include <cstring>

using namespace tyt_tool::dfu;

auto TYT::IdentifyDevice() const -> std::string
{
    auto data = ReadRegister(TYTRegister::RadioInfo);

    //model is null-terminated str, get len with strlen
    auto slen = strlen((const char *)data.data());
    return std::string(data.begin(), data.begin() + slen + 1);
}

auto TYT::ReadRegister(const TYTRegister &reg) const -> std::vector<uint8_t>
{
    InitDownload();
    Download({static_cast<uint8_t>(TYTCommand::ReadRegister),
              static_cast<uint8_t>(reg)});

    //execute command by calling GetStatus
    auto status = GetStatus();
    if (status.state != DFUState::DFU_DOWNLOAD_BUSY)
    {
        throw DFUException("Read register failed");
    } else if(status.timeout > 0) {
        std::this_thread::sleep_for(std::chrono::nanoseconds(status.timeout));
    }

    //check the command executed ok
    status = GetStatus();
    if(status.state != DFUState::DFU_DOWNLOAD_IDLE) {
        throw DFUException("Read register failed");
    }

    InitUpload();
    return Upload(TYTRegisterSize);
}

auto TYT::InitDownload() const -> void
{
    while (1)
    {
        auto state = GetState();
        switch (state)
        {
        case DFUState::DFU_DOWNLOAD_IDLE:
        case DFUState::DFU_IDLE:
            return;
        default:
        {
            Abort();
            break;
        }
        }
    }
}

auto TYT::InitUpload() const -> void
{
    while (1)
    {
        auto state = GetState();
        switch (state)
        {
        case DFUState::DFU_UPLOAD_IDLE:
        case DFUState::DFU_IDLE:
            return;
        default:
        {
            Abort();
            break;
        }
        }
    }
}