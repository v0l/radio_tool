#include <tyt_dfu.hpp>
#include <dfu_exception.hpp>

#include <chrono>
#include <thread>
#include <cstring>

using namespace tytfw::dfu;

auto TYTDFU::IdentifyDevice() const -> std::string
{
    Download({0xa2, 0x01});

    //wait for dfuDOWNLOAD_IDLE
    WaitForIdle();

    //set mode back to dfuIDLE
    Abort(); 

    //read the data
    auto data = Upload(32);

    //set mode back to dfuIDLE
    Abort();

    //model is null-terminated str, get len with strlen
    auto slen = strlen((const char*)data.data());
    return std::string(data.begin(), data.begin() + slen + 1);
}

auto TYTDFU::GetState() const -> DFUState
{
    CheckDevice();
    unsigned char state;
    auto err = libusb_control_transfer(this->device, 0xa1, static_cast<uint8_t>(DFURequest::GetState), 0, 0, &state, 1, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
    else
    {
        auto s = static_cast<DFUState>((int)state);
        //std::cerr << "State: " << ::ToString(s) << std::endl;
        return s;
    }
}

auto TYTDFU::GetStatus() const -> const DFUStatusReport
{
    CheckDevice();
    auto constexpr StatusSize = 6;

    unsigned char data[StatusSize];
    auto err = libusb_control_transfer(this->device, 0xa1, static_cast<uint8_t>(DFURequest::GetStatus), 0, 0, data, StatusSize, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
    else
    {
        return DFUStatusReport::Parse(data);
    }
    return DFUStatusReport::Empty();
}

auto TYTDFU::Abort() const -> void
{
    CheckDevice();
    auto err = libusb_control_transfer(this->device, 0x21, static_cast<uint8_t>(DFURequest::Abort), 0, 0, nullptr, 0, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto TYTDFU::Detach() const -> void {
    CheckDevice();
    auto err = libusb_control_transfer(this->device, 0x21, static_cast<uint8_t>(DFURequest::Detach), 0, 0, nullptr, 0, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto TYTDFU::WaitForIdle() const -> void
{
    while (1)
    {
        auto status = GetStatus();
        if (status.state == DFUState::dfuDOWNLOAD_IDLE)
        {
            break;
        }
        else
        {
            using namespace std::chrono_literals;
            std::this_thread::sleep_for(10ms);
        }
    }
}

auto TYTDFU::EnterDFUMode() const -> void
{
    while (1)
    {
        auto state = GetState();
        if (state == DFUState::dfuIDLE)
            return;
        
        switch (state)
        {
        case DFUState::dfuIDLE:
            return;
        case DFUState::dfuDOWNLOAD_IDLE:
        case DFUState::dfuDOWNLOAD_SYNC:
        case DFUState::dfuMANAIFEST:
        case DFUState::dfuMANIGEST_SYNC:
        case DFUState::dfuUPLOAD_IDLE:
        {
            Abort();
            break;
        }
        default:
            break;
        }
    }
}