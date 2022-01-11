/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 Niccolò Izzo IU2KIN
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
#include <fymodem.h>

#include <errno.h>
#include <fcntl.h>
#include <iomanip>
#include <iostream>
#include <math.h>
#include <string.h>
#include <vector>

#ifdef _WIN32
#else
#include <termios.h>
#include <unistd.h>
#endif

using namespace radio_tool::radio;

auto AilunceRadio::ToString() const -> const std::string
{
    std::stringstream out;

    out << "== Ailunce USB Serial Cable ==" << std::endl;

    return out.str();
}

auto AilunceRadio::SetInterfaceAttribs(int fd, int speed, int parity) const -> int
{
#ifdef _WIN32

    return 0;
#else
    struct termios tty;
    if (tcgetattr (fd, &tty) != 0)
    {
            perror("Error accessing TTY attributes");
            return -1;
    }

    cfsetospeed (&tty, speed);
    cfsetispeed (&tty, speed);

    tty.c_cflag = (tty.c_cflag & ~CSIZE) | CS8;     // 8-bit chars
    // disable IGNBRK for mismatched speed tests; otherwise receive break
    // as \000 chars
    tty.c_iflag &= ~IGNBRK;         // disable break processing
    tty.c_lflag = 0;                // no signaling chars, no echo,
                                    // no canonical processing
    tty.c_oflag = 0;                // no remapping, no delays
    tty.c_cc[VMIN]  = 0;
    tty.c_cc[VTIME] = 20;

    tty.c_iflag &= ~(IXON | IXOFF | IXANY); // shut off xon/xoff ctrl

    tty.c_cflag |= (CLOCAL | CREAD);// ignore modem controls,
                                    // enable reading
    tty.c_cflag &= ~(PARENB | PARODD);      // shut off parity
    tty.c_cflag |= parity;
    tty.c_cflag &= ~CSTOPB;
    tty.c_cflag &= ~CRTSCTS;

    if (tcsetattr (fd, TCSANOW, &tty) != 0)
    {
            perror("Error setting TTY attributes");
            return -1;
    }
    return 0;
#endif
}

auto AilunceRadio::WriteFirmware(const std::string &file, const std::string &port) const -> void
{
    constexpr auto TransferSize = 1024u;

    auto fw = fw::AilunceFW();
    fw.Read(file);
    fw.Encrypt();

#ifdef _WIN32
    auto fd = -1;

#else
    int fd = open(port.c_str(), O_RDWR | O_NOCTTY | O_SYNC);
    if (fd < 0)
    {
            perror("Error opening serial port");
            return;
    }
    SetInterfaceAttribs(fd, B57600, 0);

    write(fd, "1", 1);           // send 1 to start firmware upgrade
    usleep(1000000);      // sleep enough to transmit the 1
#endif

    auto r = fw.GetDataSegments()[0];
    int32_t s = fymodem_send(fd, (uint8_t *)r.data.data(), r.data.size(), file.c_str());
}
