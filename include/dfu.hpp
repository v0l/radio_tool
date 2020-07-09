#pragma once

#include <stdint.h>
#include <vector>
#include <string>
#include <sstream>
#include <iomanip>
#include <map>
#include <iostream>
#include <libusb-1.0/libusb.h>

namespace tytfw::dfu
{
    enum class DFURequest : uint8_t
    {
        Detach = 0,
        Download = 1,
        Upload = 2,
        GetStatus = 3,
        CLRStatus = 4,
        GetState = 5,
        Abort = 6
    };

    enum class DFUState : uint8_t
    {
        appIDLE = 0x00,
        appDETACH = 0x01,
        dfuIDLE = 0x02,
        dfuDOWNLOAD_SYNC = 0x03,
        dfuDOWNLOAD_BUSY = 0x04,
        dfuDOWNLOAD_IDLE = 0x05,
        dfuMANIGEST_SYNC = 0x06,
        dfuMANAIFEST = 0x07,
        dfuMANIFEST_WAIT_RESET = 0x08,
        dfuUPLOAD_IDLE = 0x09,
        dfuERROR = 0x0a
    };

    enum class DFUStatus : uint8_t
    {
        OK = 0x00,
        errTARGET = 0x01,
        errFILE = 0x02,
        errWRITE = 0x03,
        errERASE = 0x04,
        errCHECKERASED = 0x05,
        errPROG = 0x06,
        errVERIFY = 0x07,
        errADDRESS = 0x08,
        errNOTDONE = 0x09,
        errFIRMWARE = 0x0a,
        errVENDOR = 0x0b,
        errUSBR = 0x0c,
        errPOR = 0x0d,
        errUNKNOWN = 0x0e,
        errSTALLEDPKT = 0x0f,
    };

    static constexpr auto ToString(DFUStatus s)
    {
        switch (s)
        {
        case DFUStatus::OK:
            return "OK";
        case DFUStatus::errTARGET:
            return "errTARGET";
        case DFUStatus::errFILE:
            return "errFILE";
        case DFUStatus::errWRITE:
            return "errWRITE";
        case DFUStatus::errERASE:
            return "errERASE";
        case DFUStatus::errCHECKERASED:
            return "errCHECKERASED";
        case DFUStatus::errPROG:
            return "errPROG";
        case DFUStatus::errVERIFY:
            return "errVERIFY";
        case DFUStatus::errADDRESS:
            return "errADDRESS";
        case DFUStatus::errNOTDONE:
            return "errNOTDONE";
        case DFUStatus::errFIRMWARE:
            return "errFIRMWARE";
        case DFUStatus::errVENDOR:
            return "errVENDOR";
        case DFUStatus::errUSBR:
            return "errUSBR";
        case DFUStatus::errPOR:
            return "errPOR";
        case DFUStatus::errUNKNOWN:
            return "errUNKNOWN";
        case DFUStatus::errSTALLEDPKT:
            return "errSTALLEDPKT";
        }
        return "**UKNOWN**";
    };

    static constexpr auto ToString(DFUState s)
    {
        switch (s)
        {
        case DFUState::appIDLE:
            return "OK";
        case DFUState::appDETACH:
            return "appDETACH";
        case DFUState::dfuIDLE:
            return "dfuIDLE";
        case DFUState::dfuDOWNLOAD_SYNC:
            return "dfuDOWNLOAD_SYNC";
        case DFUState::dfuDOWNLOAD_BUSY:
            return "dfuDOWNLOAD_BUSY";
        case DFUState::dfuDOWNLOAD_IDLE:
            return "dfuDOWNLOAD_IDLE";
        case DFUState::dfuMANIGEST_SYNC:
            return "dfuMANIGEST_SYNC";
        case DFUState::dfuMANAIFEST:
            return "dfuMANAIFEST";
        case DFUState::dfuMANIFEST_WAIT_RESET:
            return "dfuMANIFEST_WAIT_RESET";
        case DFUState::dfuUPLOAD_IDLE:
            return "dfuUPLOAD_IDLE";
        case DFUState::dfuERROR:
            return "dfuERROR";
        }
        return "**UKNOWN**";
    };

    class DFUStatusReport
    {
    public:
        DFUStatus status;
        uint32_t timeout;
        DFUState state;
        uint8_t discarded;

        static auto Parse(uint8_t data[6]) -> const DFUStatusReport
        {
            return DFUStatusReport(
                static_cast<DFUStatus>((int)data[0]),
                (((data[1] << 8) | data[2]) << 8) | data[3],
                static_cast<DFUState>((int)data[4]),
                (int)data[5]);
        }

        static auto Empty() -> const DFUStatusReport
        {
            return DFUStatusReport();
        }

        auto ToString() const -> std::string
        {
            std::stringstream out;
            out << "Status: 0x" << tytfw::dfu::ToString(status) << ", "
                << "Timeout: 0x" << std::setfill('0') << std::setw(2) << std::hex << timeout << ", "
                << "State: 0x" << tytfw::dfu::ToString(state) << ", "
                << "Discarded: 0x" << std::setfill('0') << std::setw(2) << std::hex << discarded;
            return out.str();
        }

    private:
        DFUStatusReport()
            : status(DFUStatus::errUNKNOWN), timeout(0), state(DFUState::dfuERROR), discarded(0)
        {
        }
        DFUStatusReport(DFUStatus a, uint32_t b, DFUState c, uint8_t d)
            : status(a), timeout(b), state(c), discarded(d)
        {
            //std::cerr << ToString() << std::endl;
        }
    };

    class DFU
    {
    public:
        DFU(const uint16_t vid, const uint16_t pid, const uint16_t idx = 0)
            : vid(vid), pid(pid), idx(idx), timeout(500), device(nullptr) {}
        auto Init() -> bool;
        auto ListDevices() -> std::vector<std::wstring>;

        auto Open(uint16_t idx) -> bool;
        auto SetAddress(const uint32_t) const;
        auto CustomCT(const std::vector<uint8_t>) const;
        auto Erase(const uint32_t) const;
        auto Download(std::vector<uint8_t>) const -> void;
        auto Upload(const uint16_t) const -> std::vector<uint8_t>;

    private:
        libusb_context *usb_ctx;
        auto GetDeviceString(const libusb_device_descriptor &, libusb_device_handle *) const -> std::wstring;

    protected:
        const uint16_t vid, pid, idx, timeout;
        libusb_device_handle *device;
        auto CheckDevice() const -> void;
    };
} // namespace tytfw::dfu