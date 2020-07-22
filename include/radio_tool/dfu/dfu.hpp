/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 Kieran Harkin <kieran+git@harkin.me>
 * 
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * radio_tool is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with radio_tool. If not, see <https://www.gnu.org/licenses/>.
 */
#pragma once

#include <stdint.h>
#include <vector>
#include <string>
#include <sstream>
#include <iomanip>
#include <iostream>

#include <libusb-1.0/libusb.h>

namespace radio_tool::dfu
{
    enum class DFURequest : uint8_t
    {
        /**
         * Requests the device to leave DFU mode and enter the application.
         */
        DETACH = 0x00,

        /**
         * Requests data transfer from Host to the device in order to load them
         * into device internal Flash memory. Includes also erase commands.
        */
        DNLOAD = 0x01,

        /**
         * Requests data transfer from device to Host in order to load content
         * of device internal Flash memory into a Host file.
         */
        UPLOAD = 0x02,

        /**
         * Requests device to send status report to the Host (including status
         * resulting from the last request execution and the state the device will
         * enter immediately after this request).
         */
        GETSTATUS = 0x03,

        /**
         * Requests device to clear error status and move to next step.
         */
        CLRSTATUS = 0x04,

        /**
         * Requests the device to send only the state it will enter immediately
         * after this request.
         */
        GETSTATE = 5,

        /**
         * Requests device to exit the current state/operation and enter idle
         * state immediately.
         */
        ABORT = 6
    };

    enum class DFUState : uint8_t
    {
        IDLE = 0x00,
        DETACH = 0x01,
        DFU_IDLE = 0x02,
        DFU_DOWNLOAD_SYNC = 0x03,
        DFU_DOWNLOAD_BUSY = 0x04,
        DFU_DOWNLOAD_IDLE = 0x05,
        DFU_MANIFEST_SYNC = 0x06,
        DFU_MANIFEST = 0x07,
        DFU_MANIFEST_WAIT_RESET = 0x08,
        DFU_UPLOAD_IDLE = 0x09,
        DFU_ERROR = 0x0a,
        DFU_UPLOAD_SYNC = 0x91,
        DFU_UPLOAD_BUSY = 0x92
    };

    enum class DFUStatus : uint8_t
    {
        OK = 0x00,
        errTARGET = 0x01,
        errFILE = 0x02,
        errWRITE = 0x03,
        errERASE = 0x04,
        errCHECK_ERASE = 0x05,
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

    static auto ToString(DFUStatus s)
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
        case DFUStatus::errCHECK_ERASE:
            return "errCHECK_ERASE";
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

    static auto ToString(DFUState s)
    {
        switch (s)
        {
        case DFUState::IDLE:
            return "IDLE";
        case DFUState::DETACH:
            return "DETACH";
        case DFUState::DFU_IDLE:
            return "DFU_IDLE";
        case DFUState::DFU_DOWNLOAD_SYNC:
            return "DFU_DOWNLOAD_SYNC";
        case DFUState::DFU_DOWNLOAD_BUSY:
            return "DFU_DOWNLOAD_BUSY";
        case DFUState::DFU_DOWNLOAD_IDLE:
            return "DFU_DOWNLOAD_IDLE";
        case DFUState::DFU_MANIFEST_SYNC:
            return "DFU_MANIFEST_SYNC";
        case DFUState::DFU_MANIFEST:
            return "DFU_MANIFEST";
        case DFUState::DFU_MANIFEST_WAIT_RESET:
            return "DFU_MANIFEST_WAIT_RESET";
        case DFUState::DFU_UPLOAD_IDLE:
            return "DFU_UPLOAD_IDLE";
        case DFUState::DFU_ERROR:
            return "DFU_ERROR";
        case DFUState::DFU_UPLOAD_SYNC:
            return "DFU_UPLOAD_SYNC";
        case DFUState::DFU_UPLOAD_BUSY:
            return "DFU_UPLOAD_BUSY";
        }
        return "**UKNOWN**";
    };

    class DFUStatusReport
    {
    public:
        const DFUStatus status;
        const uint32_t timeout;
        const DFUState state;
        const uint8_t discarded;

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
            out << "Status: " << radio_tool::dfu::ToString(status) << ", "
                << "Timeout: 0x" << std::setfill('0') << std::setw(2) << std::hex << timeout << ", "
                << "State: " << radio_tool::dfu::ToString(state) << ", "
                << "Discarded: 0x" << std::setfill('0') << std::setw(2) << std::hex << discarded;
            return out.str();
        }

    private:
        DFUStatusReport()
            : status(DFUStatus::errUNKNOWN), timeout(0), state(DFUState::DFU_ERROR), discarded(0)
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
        DFU(libusb_device_handle *device)
            : timeout(5000), device(device) {}

        auto SetAddress(const uint32_t) const -> void;
        auto Erase(const uint32_t) const -> void;
        auto Download(const std::vector<uint8_t> &, const uint16_t wValue = 0) const -> void;
        auto Upload(const uint16_t, const uint8_t wValue = 0) const -> std::vector<uint8_t>;

        auto Get() const -> std::vector<uint8_t>;
        auto ReadUnprotected() const -> void;

        auto GetState() const -> DFUState;
        auto GetStatus() const -> const DFUStatusReport;
        auto Abort() const -> void;
        auto Detach() const -> void;

    private:
        libusb_context *usb_ctx;
        auto GetDeviceString(const libusb_device_descriptor &, libusb_device_handle *) const -> std::wstring;

    protected:
        const uint16_t timeout;
        libusb_device_handle *device;

        auto CheckDevice() const -> void;
        
        /**
         * Ensures the state is DFU_IDLE or DFU_DNLOAD_IDLE
         */
        auto InitDownload() const -> void;

        /**
         * Ensures the state is DFU_IDLE or DFU_DPLOAD_IDLE
         */
        auto InitUpload() const -> void;
    };
} // namespace radio_tool::dfu