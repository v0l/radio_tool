/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 v0l <radio_tool@v0l.io>
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
#include <radio_tool/dfu/dfu.hpp>
#include <radio_tool/dfu/dfu_exception.hpp>

#include <exception>
#include <thread>
#include <chrono>

using namespace radio_tool::dfu;

auto DFU::SetAddress(const uint32_t &addr) const -> void
{
    std::vector<uint8_t> data = {
        static_cast<uint8_t>(0x21),
        static_cast<uint8_t>(addr & 0xFF),
        static_cast<uint8_t>((addr >> 8) & 0xFF),
        static_cast<uint8_t>((addr >> 16) & 0xFF),
        static_cast<uint8_t>((addr >> 24) & 0xFF)};

    Download(data);
}

auto DFU::Erase(const uint32_t &addr) const -> void
{
    std::vector<uint8_t> data = {
        static_cast<uint8_t>(0x41),
        static_cast<uint8_t>(addr & 0xFF),
        static_cast<uint8_t>((addr >> 8) & 0xFF),
        static_cast<uint8_t>((addr >> 16) & 0xFF),
        static_cast<uint8_t>((addr >> 24) & 0xFF)};

    Download(data);
}

auto DFU::Download(const std::vector<uint8_t>& data, const uint16_t &wValue) const -> void
{
    InitDownload();
    // tehnically we shouldnt const_cast here but libusb *?WONT?* modify this data
    auto err = libusb_control_transfer(device, 0x21, static_cast<uint8_t>(DFURequest::DNLOAD), wValue, 0, const_cast<unsigned char*>(data.data()), data.size(), this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }

    //execute command by calling GetStatus
    auto status = GetStatus();
    if (status.state != DFUState::DFU_DOWNLOAD_BUSY)
    {
        throw DFUException("Command execution failed");
    } else if(status.timeout > 0) {
        //std::this_thread::sleep_for(std::chrono::nanoseconds(status.timeout));
    }

    //check the command executed ok
    auto status2 = GetStatus();
    if(status2.state != DFUState::DFU_DOWNLOAD_IDLE) {
        throw DFUException("Command execution failed");
    }
}

auto DFU::Upload(const uint16_t &size, const uint8_t &wValue) const -> std::vector<uint8_t>
{
    InitUpload();
    auto data = std::vector<uint8_t>(size);
    auto err = libusb_control_transfer(device, 0xa1, static_cast<uint8_t>(DFURequest::UPLOAD), wValue, 0, data.data(), data.size(), this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
    else
    {
        data.resize(err);
    }
    return data;
}


auto DFU::GetState() const -> DFUState
{
    CheckDevice();
    unsigned char state;
    auto err = libusb_control_transfer(device, 0xa1, static_cast<uint8_t>(DFURequest::GETSTATE), 0, 0, &state, 1, this->timeout);
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

auto DFU::GetStatus() const -> const DFUStatusReport
{
    CheckDevice();
    auto constexpr StatusSize = 6;

    unsigned char data[StatusSize];
    auto err = libusb_control_transfer(device, 0xa1, static_cast<uint8_t>(DFURequest::GETSTATUS), 0, 0, data, StatusSize, this->timeout);
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

auto DFU::Abort() const -> void
{
    CheckDevice();
    auto err = libusb_control_transfer(device, 0x21, static_cast<uint8_t>(DFURequest::ABORT), 0, 0, nullptr, 0, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto DFU::Detach() const -> void {
    CheckDevice();
    auto err = libusb_control_transfer(device, 0x21, static_cast<uint8_t>(DFURequest::DETACH), 0, 0, nullptr, 0, this->timeout);
    if (err < LIBUSB_SUCCESS)
    {
        throw DFUException(libusb_error_name(err));
    }
}

auto DFU::CheckDevice() const -> void
{
    if (this->device == nullptr)
        throw std::runtime_error("Device is not opened");
}



auto DFU::InitDownload() const -> void
{
    CheckDevice();
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

auto DFU::InitUpload() const -> void
{
    CheckDevice();
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