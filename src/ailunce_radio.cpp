/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 Niccol� Izzo IU2KIN
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
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
#include <radio_tool/radio/ailunce_radio.hpp>
#include <radio_tool/fw/ailunce_fw.hpp>

#include <thread>

#ifdef _WIN32
#include <Windows.h>
#include <io.h>
#include <iostream>
#include <regex>

#ifdef COMPORT_DI_LOOKUP
#pragma comment(lib, "Setupapi.lib")
#include <SetupAPI.h>
#else
#endif

#else
#include <unistd.h>
#endif

using namespace radio_tool::radio;

auto AilunceRadio::ToString() const -> const std::string
{
    return "== Ailunce USB Serial Cable ==";
}

auto AilunceRadio::WriteFirmware(const std::string &file) const -> void
{
    auto fw = fw::AilunceFW();
    fw.Read(file);

    //XOR raw binary data before sending
    fw.Encrypt();

    auto fd = device.GetFD();
    // send 1 to start firmware upgrade
#ifdef _WIN32
    WriteFile((HANDLE)fd, "1", (DWORD)1, NULL, NULL);
#else
    write(fd, "1", 1);
#endif
    std::this_thread::sleep_for(std::chrono::milliseconds(100));

    auto r = fw.GetDataSegments()[0];
    device.SetInterfaceAttribs(57600, 0);
    device.Write(r.data);
}

auto AilunceRadio::SupportsDevice(const std::string &port) -> bool
{
    // not possible to detect from serial port?
    // ideally we could map serial ports to USB devices to validate VID:PID
    //
    // ✅ possible windows solution: https://aticleworld.com/get-com-port-of-usb-serial-device/
    // possible linux solution: https://unix.stackexchange.com/a/81767
    auto ids = GetComPortUSBIds(port);
    return (ids.first == VID && ids.second == PID) || true;
}

auto AilunceRadio::GetComPortUSBIds(const std::string &port) -> std::pair<uint16_t, uint16_t>
{
#if defined(_WIN32) && defined(COMPORT_DI_LOOKUP)
    auto handle = SetupDiGetClassDevs(NULL, "USB", NULL, DIGCF_ALLCLASSES | DIGCF_PRESENT);
    if (handle == nullptr)
    {
        throw std::runtime_error("Failed to open device info");
    }

    BYTE hwIds[1024];
    DEVPROPTYPE propType;
    SP_DEVINFO_DATA deviceInfo = {};
    deviceInfo.cbSize = sizeof(SP_DEVINFO_DATA);

    DWORD idx = 0, outSize = 0;
    while (SetupDiEnumDeviceInfo(handle, idx++, &deviceInfo))
    {
        if (SetupDiGetDeviceRegistryPropertyA(handle, &deviceInfo, SPDRP_HARDWAREID, &propType, (PBYTE)&hwIds, sizeof(hwIds), &outSize))
        {
            std::cerr << hwIds << std::endl;

            HKEY regKey;
            if ((regKey = SetupDiOpenDevRegKey(handle, &deviceInfo, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ)) == INVALID_HANDLE_VALUE)
            {
                std::cerr << "Failed to find reg key for device: " << (char *)hwIds << std::endl;
            }

            // read com port name
            constexpr auto BufferLen = 256;
            DWORD dwType = 0;
            DWORD portNameSize = BufferLen;
            BYTE portName[BufferLen];
            if (RegQueryValueExA(regKey, "PortName", NULL, &dwType, (LPBYTE)&portName, &portNameSize) == ERROR_SUCCESS)
            {
                // not working for some reason
                std::cerr << portName << std::endl;
            }
        }
    }

    SetupDiDestroyDeviceInfoList(handle);
#elif defined(_WIN32)
    HKEY comKey = nullptr;
    auto openResult = RegOpenKeyExA(HKEY_LOCAL_MACHINE, (LPSTR) "SYSTEM\\CurrentControlSet\\Control\\COM Name Arbiter\\Devices", 0, KEY_READ | KEY_WOW64_64KEY, &comKey);
    if (openResult != ERROR_SUCCESS)
    {
        throw std::runtime_error("Failed to get serial port info from registry");
    }

    constexpr auto BufferSize = 256L;
    BYTE value[BufferSize];
    DWORD valueSize = BufferSize;

    auto readResult = RegQueryValueExA(comKey, port.c_str(), NULL, NULL, (LPBYTE)&value, &valueSize);
    if (readResult == ERROR_SUCCESS)
    {
        std::cmatch match;
        if (std::regex_match((char *)value, match, std::regex("^.*#vid_([\\w+]{4})\\+pid_([\\w+]{4}).*$")))
        {
            auto vid = std::stoi(match[1], nullptr, 16);
            auto pid = std::stoi(match[2], nullptr, 16);
            return std::make_pair(vid, pid);
        }
    }
    else
    {
        throw std::runtime_error("Error reading registory key values");
    }

    RegCloseKey(comKey);
#else

#endif
    return std::make_pair(0, 0);
}