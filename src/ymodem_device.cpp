/**
 * This file is part of radio_tool.
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
#include <radio_tool/device/ymodem_device.hpp>

#include <fymodem.h>
#include <stdio.h>
#include <fcntl.h>

#ifdef _WIN32
#else
#include <termios.h>
#endif

using namespace radio_tool::device;

YModemDevice::YModemDevice(const std::string &port, const std::string &filename) : port(port), filename(filename), fd(-1)
{
    int fdOpen = open(port.c_str(), O_RDWR | O_NOCTTY | O_SYNC);
    if (fdOpen < 0)
    {
        throw std::runtime_error("Failed to open port");
    }

    fd = fdOpen;
}

auto YModemDevice::SetAddress(const uint32_t &) const -> void
{
    throw std::runtime_error("SetAddress not supported for YModem device");
}

auto YModemDevice::Erase(const uint32_t &) const -> void
{
    throw std::runtime_error("Erase not supported for YModem device");
}

auto YModemDevice::Write(const std::vector<uint8_t> &data) const -> void
{
    auto fn = std::string(filename);
    size_t wlen = fymodem_send(fd, (uint8_t *)data.data(), data.size(), fn.data());
    if (wlen != data.size())
    {
        throw std::runtime_error("Write error");
    }
}

auto YModemDevice::Read(const uint16_t &size) const -> std::vector<uint8_t>
{
    auto ret = std::vector<uint8_t>();
    ret.resize(size);

    auto fn = std::string(filename);
    auto rsize = fymodem_receive(ret.data(), size, fn.data());
    if (rsize != size)
    {
        throw std::runtime_error("Read error");
    }
    return ret;
}

auto YModemDevice::Status() const -> const std::string
{
    return "OK";
}

auto YModemDevice::SetInterfaceAttribs(const uint32_t &speed, const int &parity) const -> int
{
#ifdef _WIN32

    return -1;
#else
    struct termios tty;
    if (tcgetattr(fd, &tty) != 0)
    {
        throw std::runtime_error("Error accessing TTY attributes");
        return -1;
    }

    cfsetospeed(&tty, speed);
    cfsetispeed(&tty, speed);

    tty.c_cflag = (tty.c_cflag & ~CSIZE) | CS8; // 8-bit chars
    // disable IGNBRK for mismatched speed tests; otherwise receive break
    // as \000 chars
    tty.c_iflag &= ~IGNBRK; // disable break processing
    tty.c_lflag = 0;        // no signaling chars, no echo,
                            // no canonical processing
    tty.c_oflag = 0;        // no remapping, no delays
    tty.c_cc[VMIN] = 0;
    tty.c_cc[VTIME] = 20;

    tty.c_iflag &= ~(IXON | IXOFF | IXANY); // shut off xon/xoff ctrl

    tty.c_cflag |= (CLOCAL | CREAD);   // ignore modem controls,
                                       // enable reading
    tty.c_cflag &= ~(PARENB | PARODD); // shut off parity
    tty.c_cflag |= parity;
    tty.c_cflag &= ~CSTOPB;
    tty.c_cflag &= ~CRTSCTS;

    if (tcsetattr(fd, TCSANOW, &tty) != 0)
    {
        throw std::runtime_error("Error setting TTY attributes");
        return -1;
    }
    return 0;
#endif
}