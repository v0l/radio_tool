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
#include <chrono>
#include <vector>
#include <array>
#include <string>
#include <cstring>
#include <cstdint>
#include <stdexcept>
#include <fstream>
#include <iostream>
#include <iomanip>

#ifdef _WIN32
#define B57600 57600
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
#include <termios.h>
#include <fcntl.h>
#include <errno.h>
#include <sys/time.h>
#include <sys/select.h>
#include <sys/ioctl.h>
#if defined(__APPLE__)
#include <IOKit/serial/ioss.h>
#endif
#endif

using namespace radio_tool::radio;

auto AilunceRadio::ToString() const -> const std::string
{
    return "== Ailunce USB Serial Cable ==";
}

auto AilunceRadio::WriteFirmware(const std::string &file) -> void
{
    auto fw = fw::AilunceFW();
    fw.Read(file);

    //XOR raw binary data before sending
    fw.Encrypt();
    
    device.SetInterfaceAttribs(B57600, 0);
    auto fd = device.GetFD();
    // send 1 to start firmware upgrade
#ifdef _WIN32
    WriteFile((HANDLE)fd, "1", (DWORD)1, NULL, NULL);
#else
    write(fd, "1", 1);
#endif
    std::this_thread::sleep_for(std::chrono::milliseconds(100));

    auto r = fw.GetDataSegments()[0];
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

// ===========================================================================
// Ailunce HD2 codeplug CPS protocol.
//
// Ported byte-for-byte from vendor/dmrconfig/hd2.c (hd2_download / hd2_upload /
// hd2_csum / hd2_build_read / hd2_build_write_0f / hd2_build_write_31 /
// hd2_write_ack / HD2_REGIONS[]), which is validated on real hardware against
// both the vendor firmware (119200 baud) and OpenRTX (57600 baud).  The
// reference pylunce scripts are hd1_dump.py / hd1_codeplug_write.py.
//
// All serial I/O goes through the device fd (device.GetFD()) using POSIX
// read()/write()/select() -- the same fd the firmware path writes "1" to.
// ===========================================================================
namespace
{
    constexpr unsigned HD2_MEMSZ    = 0xE5000; // 938000 bytes -- full image size
    constexpr uint8_t  HD2_SYNC     = 0x68;
    constexpr uint8_t  HD2_TERM     = 0x10;
    constexpr unsigned HD2_CHUNK_0F = 0x80;    // 128-byte byte-addressed unit
    constexpr unsigned HD2_BLOCK_31 = 0x1000;  // 4096-byte block-write unit
    constexpr unsigned HD2_READBLK  = 0x400;   // 1024-byte block-read unit

    // Codeplug regions, in dump order.  Mirrors HD2_REGIONS[] in dmrconfig
    // hd2.c; the lengths sum to exactly HD2_MEMSZ.  For is_block regions
    // radio_base is a 1024-byte block index, otherwise it is a byte address.
    struct HD2Region
    {
        const char *name;
        unsigned    radio_base;
        unsigned    file_base;
        unsigned    length;
        unsigned    unit;     // 0x80 (0x0F reads) or 0x400 (0x31 reads)
        bool        is_block;
    };

    constexpr HD2Region HD2_REGIONS[] = {
        { "header",     0x0000, 0x000000, 0x00200, 0x80,  false },
        { "vfo_config", 0x2000, 0x000200, 0x00380, 0x80,  false },
        { "settings",   0x2900, 0x000580, 0x03680, 0x80,  false },
        { "addr_book",  0x0000, 0x003C00, 0x00400, 0x400, true  },
        { "channels",   0x1B84, 0x004000, 0x93000, 0x400, true  },
        { "table_1dfx", 0x1DF8, 0x097000, 0x03000, 0x400, true  },
        { "table_21dx", 0x21D8, 0x09A000, 0x4A000, 0x400, true  },
        { "table_4000", 0x4000, 0x0E4000, 0x00400, 0x400, true  },
        { "table_428x", 0x4280, 0x0E4400, 0x00C00, 0x400, true  },
    };

    // --- delta-write manifest ----------------------------------------------
    //
    // The dirty-chunk write needs to know which chunks already match the radio.
    // We keep a manifest of per-chunk CRC32s in the codeplug image itself, in a
    // reserved 4 KB block, so it round-trips through the radio's flash and is
    // available on the next connection.
    //
    // Placement: file offset 0x96000 -- the last 4 KB write-block of the
    // "channels" region (0x4000..0x96FFF).  We keep the manifest inside a known
    // region (channels) rather than an unmapped one, and pay for it by
    // sacrificing the top channel slots that this block overlaps: vendor slots
    // ~2978..3001 == real channels ~2976..2999.  No real codeplug uses that
    // many channels, but we do not *assume* the slots are free -- before
    // writing the manifest we check the codeplug's own presence bitmap and
    // refuse if any of those slots actually holds a channel (see
    // hd2_check_manifest_region_free()).  The block is read and written exactly
    // like any other channels block, so the manifest survives a round-trip.
    // Layout in that block:
    //     +0x00  4 B   magic  "HD2M"
    //     +0x04  4 B   version (LE)  = 1
    //     +0x08  4 B   chunk count (LE)
    //     +0x0C  N*4 B per-chunk CRC32 (LE), in chunk-enumeration order
    // If the magic is absent/wrong the manifest is treated as invalid and a
    // full write is performed (rebuilding the manifest from scratch).
    constexpr unsigned HD2_MANIFEST_OFF = 0x96000;
    constexpr unsigned HD2_MANIFEST_LEN = HD2_BLOCK_31;            // 4096 bytes reserved
    constexpr uint32_t HD2_MANIFEST_MAGIC = 0x4D324448;            // "HD2M" little-endian
    constexpr uint32_t HD2_MANIFEST_VERSION = 1;

    // Channel-slot geometry, for the manifest-region safety check.  Vendor slot
    // s lives at file HD2_CHAN_BASE + s*HD2_CHAN_STRIDE; slots 0/1 are the
    // VFO-A/B presets, so real (CPS/OpenRTX) channel index i == vendor slot
    // i+2.  The presence bitmap (bm1) starts at file HD2_BITMAP_OFF and is
    // indexed by real channel; a CLEAR bit means the channel is populated.
    constexpr unsigned HD2_CHAN_BASE   = 0x16080;
    constexpr unsigned HD2_CHAN_STRIDE = 176;
    constexpr unsigned HD2_BITMAP_OFF  = 0x200;

    // CRC32 (IEEE 802.3, reflected, poly 0xEDB88820) -- the same CRC32 the
    // delta manifest stores per chunk.
    auto crc32(const uint8_t *data, size_t len) -> uint32_t
    {
        uint32_t crc = 0xFFFFFFFFu;
        for (size_t i = 0; i < len; i++)
        {
            crc ^= data[i];
            for (int b = 0; b < 8; b++)
                crc = (crc >> 1) ^ (0xEDB88820u & (~(crc & 1) + 1));
        }
        return ~crc;
    }

    // Command checksum -- two formulas selected by b1 (see dmrconfig hd2_csum).
    auto hd2_csum(int b1, unsigned addr, unsigned size) -> uint8_t
    {
        if (b1 == 0x31)
            return static_cast<uint8_t>((0xFF - (0x15 + ((addr - 0x88) >> 8))) & 0xFF);

        unsigned addr_lo = addr & 0xFF;
        unsigned addr_hi = (addr >> 8) & 0xFF;
        return static_cast<uint8_t>((0xFF - (0x10 + addr_hi + ((addr_lo + size - 1) >> 7))) & 0xFF);
    }

    // 11-byte read-request frame:
    //   68 b1 00 01 [pct] [csum] [size_lo] [size_hi] [addr_lo] [addr_hi] 10
    auto hd2_build_read(int b1, unsigned addr, unsigned size, int pct) -> std::array<uint8_t, 11>
    {
        std::array<uint8_t, 11> cmd{};
        cmd[0]  = HD2_SYNC;
        cmd[1]  = static_cast<uint8_t>(b1);
        cmd[2]  = 0x00;
        cmd[3]  = 0x01;
        cmd[4]  = static_cast<uint8_t>(pct & 0xFF);
        cmd[5]  = hd2_csum(b1, addr, size);
        cmd[6]  = static_cast<uint8_t>(size & 0xFF);
        cmd[7]  = static_cast<uint8_t>((size >> 8) & 0xFF);
        cmd[8]  = static_cast<uint8_t>(addr & 0xFF);
        cmd[9]  = static_cast<uint8_t>((addr >> 8) & 0xFF);
        cmd[10] = HD2_TERM;
        return cmd;
    }

    // b1=0x0F write frame (128-byte chunk):
    //   68 0f 01 01 [pct] 00 80 00 [alo][ahi] [128B] 10   (139 bytes)
    auto hd2_build_write_0f(int pct, unsigned addr, const uint8_t *data) -> std::vector<uint8_t>
    {
        std::vector<uint8_t> cmd(11 + HD2_CHUNK_0F);
        cmd[0] = HD2_SYNC;
        cmd[1] = 0x0F;
        cmd[2] = 0x01; // write
        cmd[3] = 0x01;
        cmd[4] = static_cast<uint8_t>(pct & 0xFF);
        cmd[5] = 0x00;
        cmd[6] = static_cast<uint8_t>(HD2_CHUNK_0F & 0xFF);
        cmd[7] = static_cast<uint8_t>((HD2_CHUNK_0F >> 8) & 0xFF);
        cmd[8] = static_cast<uint8_t>(addr & 0xFF);
        cmd[9] = static_cast<uint8_t>((addr >> 8) & 0xFF);
        std::memcpy(cmd.data() + 10, data, HD2_CHUNK_0F);
        cmd[10 + HD2_CHUNK_0F] = HD2_TERM;
        return cmd;
    }

    // b1=0x31 write frame (4096-byte block):
    //   68 31 01 01 [pct] 31 00 10 [alo][ahi] [4096B] 10  (4107 bytes)
    auto hd2_build_write_31(int pct, unsigned addr, const uint8_t *data) -> std::vector<uint8_t>
    {
        std::vector<uint8_t> cmd(11 + HD2_BLOCK_31);
        cmd[0] = HD2_SYNC;
        cmd[1] = 0x31;
        cmd[2] = 0x01; // write
        cmd[3] = 0x01;
        cmd[4] = static_cast<uint8_t>(pct & 0xFF);
        cmd[5] = 0x31; // constant
        cmd[6] = 0x00; // size_lo
        cmd[7] = 0x10; // size_hi (0x1000 = 4096)
        cmd[8] = static_cast<uint8_t>(addr & 0xFF);
        cmd[9] = static_cast<uint8_t>((addr >> 8) & 0xFF);
        std::memcpy(cmd.data() + 10, data, HD2_BLOCK_31);
        cmd[10 + HD2_BLOCK_31] = HD2_TERM;
        return cmd;
    }

#ifndef _WIN32
    // --- low-level serial helpers (POSIX) -----------------------------------

    auto serial_write_all(int fd, const uint8_t *buf, size_t len) -> void
    {
        size_t off = 0;
        while (off < len)
        {
            auto n = ::write(fd, buf + off, len - off);
            if (n < 0)
            {
                if (errno == EINTR)
                    continue;
                throw std::runtime_error("Serial write failed");
            }
            off += static_cast<size_t>(n);
        }
    }

    // Read up to (want) bytes into buf, retrying until satisfied or the total
    // timeout passes.  Returns the count actually read.  Bulk reads (not
    // byte-by-byte) avoid corruption at high baud on macOS.
    auto serial_read_exact(int fd, uint8_t *buf, int want, int total_timeout_msec) -> int
    {
        int got = 0;
        int idle = 0;
        while (got < want)
        {
            fd_set rfds;
            FD_ZERO(&rfds);
            FD_SET(fd, &rfds);
            timeval tv{};
            tv.tv_sec = 0;
            tv.tv_usec = 200 * 1000; // 200 ms slices
            int sel = ::select(fd + 1, &rfds, nullptr, nullptr, &tv);
            if (sel > 0 && FD_ISSET(fd, &rfds))
            {
                auto n = ::read(fd, buf + got, want - got);
                if (n > 0)
                {
                    got += static_cast<int>(n);
                    idle = 0;
                    continue;
                }
            }
            idle += 200;
            if (idle >= total_timeout_msec)
                break;
        }
        return got;
    }

    // Drain any pending input (e.g. the radio's boot debug log) until quiet.
    auto serial_drain(int fd) -> void
    {
        uint8_t buf[1024];
        int quiet = 0;
        while (quiet < 400)
        {
            fd_set rfds;
            FD_ZERO(&rfds);
            FD_SET(fd, &rfds);
            timeval tv{};
            tv.tv_sec = 0;
            tv.tv_usec = 200 * 1000;
            int sel = ::select(fd + 1, &rfds, nullptr, nullptr, &tv);
            if (sel > 0 && FD_ISSET(fd, &rfds) && ::read(fd, buf, sizeof(buf)) > 0)
                quiet = 0;
            else
                quiet += 200;
        }
    }
#endif // !_WIN32

    auto sleep_ms(int ms) -> void
    {
        std::this_thread::sleep_for(std::chrono::milliseconds(ms));
    }
} // namespace

#ifndef _WIN32
// Configure the fd for raw 8N1 at the requested baud.  Standard rates use the
// termios Bxxxxx constants; non-standard rates (e.g. 119200, the vendor fw
// rate) fall back to the platform custom-baud ioctl, exactly like dmrconfig's
// serial_open_baud().
static auto hd2_serial_configure(int fd, uint32_t baud) -> void
{
    speed_t code = 0;
    bool standard = true;
    switch (baud)
    {
    case 9600:   code = B9600;   break;
    case 19200:  code = B19200;  break;
    case 38400:  code = B38400;  break;
    case 57600:  code = B57600;  break;
    case 115200: code = B115200; break;
    default:     standard = false; break;
    }

    struct termios tty;
    std::memset(&tty, 0, sizeof(tty));
    tty.c_cflag = CS8 | CLOCAL | CREAD;
    tty.c_iflag = IGNBRK;
    tty.c_oflag = 0;
    tty.c_lflag = 0;
    tty.c_cc[VTIME] = 0;
    tty.c_cc[VMIN] = 1;

    if (standard)
    {
        cfsetispeed(&tty, code);
        cfsetospeed(&tty, code);
    }
    tcflush(fd, TCIFLUSH);
    if (tcsetattr(fd, TCSANOW, &tty) < 0)
    {
        // macOS can reject a non-standard speed left in the struct: zero the
        // speed fields and rely on the custom-baud ioctl below.
        cfsetispeed(&tty, B9600);
        cfsetospeed(&tty, B9600);
        tcsetattr(fd, TCSANOW, &tty);
    }

    if (!standard)
    {
#if defined(__APPLE__)
        speed_t s = baud;
        if (ioctl(fd, IOSSIOSPEED, &s) < 0)
            throw std::runtime_error("Cannot set custom baud rate on serial port");
#elif defined(__linux__) && defined(TCGETS2)
        struct termios2 t2;
        if (ioctl(fd, TCGETS2, &t2) < 0)
            throw std::runtime_error("TCGETS2 failed on serial port");
        t2.c_cflag &= ~CBAUD;
        t2.c_cflag |= BOTHER;
        t2.c_ispeed = baud;
        t2.c_ospeed = baud;
        if (ioctl(fd, TCSETS2, &t2) < 0)
            throw std::runtime_error("Cannot set custom baud rate on serial port");
#else
        throw std::runtime_error("Non-standard baud rate not supported on this platform");
#endif
    }
}
#endif // !_WIN32

// Open the GetVer session and verify the radio responds (used for read and as
// part of the write handshake).  Accepts any non-empty reply, matching
// dmrconfig hd2_getver().
#ifndef _WIN32
static auto hd2_getver(int fd) -> void
{
    serial_write_all(fd, reinterpret_cast<const uint8_t *>("GetVer"), 6);
    uint8_t reply[256];
    int got = serial_read_exact(fd, reply, sizeof(reply), 800);
    if (got <= 0)
    {
        // One retry: the radio may still have been emitting its boot log.
        serial_write_all(fd, reinterpret_cast<const uint8_t *>("GetVer"), 6);
        got = serial_read_exact(fd, reply, sizeof(reply), 800);
    }
    if (got <= 0)
        throw std::runtime_error("GetVer failed -- radio not responding");
}

// Send a write frame and verify the 11-byte ACK echo (sync + terminator +
// address bytes), retrying up to 4 times.  Matches dmrconfig hd2_write_ack().
static auto hd2_write_ack(int fd, const uint8_t *cmd, int cmdlen, unsigned addr) -> bool
{
    for (int attempt = 0; attempt < 4; attempt++)
    {
        serial_write_all(fd, cmd, cmdlen);
        uint8_t ack[11];
        int got = serial_read_exact(fd, ack, 11, 2000);
        if (got == 11 && ack[0] == HD2_SYNC && ack[10] == HD2_TERM &&
            ack[8] == (addr & 0xFF) && ack[9] == ((addr >> 8) & 0xFF))
            return true;
        serial_drain(fd);
    }
    return false;
}

// Read one region unit (read frame -> 10-byte echo + unit data + 0x10), with
// up to 4 retries.  Copies the unit payload into out.  Matches the inner loop
// of dmrconfig hd2_download().
static auto hd2_read_unit(int fd, int b1, unsigned addr, unsigned unit, int pct,
                          uint8_t *out, const char *region) -> void
{
    auto cmd = hd2_build_read(b1, addr, unit, pct);
    int expected = 10 + static_cast<int>(unit) + 1;
    std::vector<uint8_t> reply(static_cast<size_t>(expected));

    int got = 0;
    for (int attempt = 0; attempt < 4; attempt++)
    {
        serial_write_all(fd, cmd.data(), cmd.size());
        got = serial_read_exact(fd, reply.data(), expected, 1500);
        if (got == expected && reply[expected - 1] == HD2_TERM)
            break;
        serial_drain(fd);
    }
    if (got != expected || reply[expected - 1] != HD2_TERM)
    {
        std::stringstream ss;
        ss << "Codeplug read failed at region '" << region << "' addr 0x"
           << std::hex << addr << " (got " << std::dec << got << "/" << expected << ")";
        throw std::runtime_error(ss.str());
    }
    std::memcpy(out, reply.data() + 10, unit);
}

// Download the full codeplug image from the radio into image[] (HD2_MEMSZ).
static auto hd2_download_image(int fd, std::vector<uint8_t> &image) -> void
{
    sleep_ms(200);
    serial_drain(fd);
    hd2_getver(fd);

    // Start from all-0xFF so any unread gap reads as erased flash.
    image.assign(HD2_MEMSZ, 0xFF);

    for (const auto &reg : HD2_REGIONS)
    {
        int b1 = reg.is_block ? 0x31 : 0x0F;
        for (unsigned off = 0; off < reg.length; off += reg.unit)
        {
            unsigned addr = reg.is_block ? reg.radio_base + off / HD2_READBLK
                                         : reg.radio_base + off;
            int pct = static_cast<int>(static_cast<long long>(off) * 100 / reg.length);
            hd2_read_unit(fd, b1, addr, reg.unit, pct, image.data() + reg.file_base + off, reg.name);
        }
        std::cerr << "." << std::flush;
    }
    std::cerr << std::endl;
}

// Read just the 4 KB manifest block from the radio (not the whole image).
// The manifest already holds the CRCs of what is on the radio, so a delta
// write only needs this block -- avoiding the ~160 s full-image read at
// 57600.  out is filled with the 4 KB block (0xFF for any unread gap).
static auto hd2_read_manifest_block(int fd, std::vector<uint8_t> &out) -> void
{
    out.assign(HD2_MANIFEST_LEN, 0xFF);
    sleep_ms(200);
    serial_drain(fd);
    hd2_getver(fd);

    for (const auto &reg : HD2_REGIONS)
    {
        if (HD2_MANIFEST_OFF < reg.file_base ||
            HD2_MANIFEST_OFF >= reg.file_base + reg.length)
            continue;
        int b1 = reg.is_block ? 0x31 : 0x0F;
        for (unsigned m = 0; m < HD2_MANIFEST_LEN; m += reg.unit)
        {
            unsigned region_off = (HD2_MANIFEST_OFF - reg.file_base) + m;
            unsigned addr = reg.is_block ? reg.radio_base + region_off / HD2_READBLK
                                         : reg.radio_base + region_off;
            hd2_read_unit(fd, b1, addr, reg.unit, 0, out.data() + m, reg.name);
        }
        return;
    }
    throw std::runtime_error("manifest block offset is not inside any region");
}

// SLC7000 write handshake (sent twice, like the vendor CPS); accept any reply
// that identifies the radio family ("HD" or "BJDR").  Matches dmrconfig
// hd2_upload().
static auto hd2_write_handshake(int fd) -> void
{
    sleep_ms(200);
    serial_drain(fd);

    uint8_t reply[64];
    serial_write_all(fd, reinterpret_cast<const uint8_t *>("SLC7000"), 7);
    serial_read_exact(fd, reply, sizeof(reply), 500);
    serial_write_all(fd, reinterpret_cast<const uint8_t *>("SLC7000"), 7);
    int got = serial_read_exact(fd, reply, sizeof(reply), 800);

    bool ok = false;
    for (int i = 0; i + 1 < got; i++)
    {
        if (reply[i] == 'H' && reply[i + 1] == 'D')
            ok = true;
        if (i + 3 < got && std::memcmp(reply + i, "BJDR", 4) == 0)
            ok = true;
    }
    if (!ok)
        throw std::runtime_error("SLC7000 handshake failed -- radio not responding");
}

// Write a single 0x0F (128-byte) chunk by file offset.
static auto hd2_write_chunk_0f(int fd, unsigned radio_addr, int pct,
                               const uint8_t *data, const char *region) -> void
{
    auto cmd = hd2_build_write_0f(pct, radio_addr, data);
    if (!hd2_write_ack(fd, cmd.data(), static_cast<int>(cmd.size()), radio_addr))
    {
        std::stringstream ss;
        ss << "Codeplug write failed at region '" << region << "' addr 0x" << std::hex << radio_addr;
        throw std::runtime_error(ss.str());
    }
}

// Write a single 0x31 (4096-byte) block; data must be 4096 bytes.
static auto hd2_write_block_31(int fd, unsigned radio_addr, int pct,
                               const uint8_t *data, const char *region) -> void
{
    auto cmd = hd2_build_write_31(pct, radio_addr, data);
    if (!hd2_write_ack(fd, cmd.data(), static_cast<int>(cmd.size()), radio_addr))
    {
        std::stringstream ss;
        ss << "Codeplug write failed at region '" << region << "' block 0x" << std::hex << radio_addr;
        throw std::runtime_error(ss.str());
    }
}

// Commit + reboot.  Without END the radio drops the session and reverts.
static auto hd2_write_end(int fd) -> void
{
    serial_write_all(fd, reinterpret_cast<const uint8_t *>("END"), 3);
    sleep_ms(500);
}

// --- chunk enumeration (shared by full write + delta) ----------------------
//
// A "chunk" is one write unit: a 128-byte 0x0F chunk for byte-addressed
// regions, or a 4096-byte 0x31 block for block regions.  Enumerating in a
// single stable order lets the manifest store one CRC32 per chunk by index.
struct HD2Chunk
{
    const HD2Region *reg;
    unsigned file_off;   // absolute file offset of the chunk start
    unsigned radio_addr; // radio address for the write frame
    unsigned len;        // payload length actually backed by the file (<= unit)
    bool     is_block;   // true => 0x31 (4096), false => 0x0F (128)
};

static auto hd2_enumerate_chunks() -> std::vector<HD2Chunk>
{
    std::vector<HD2Chunk> chunks;
    for (const auto &reg : HD2_REGIONS)
    {
        if (reg.is_block)
        {
            for (unsigned off = 0; off < reg.length; off += HD2_BLOCK_31)
            {
                unsigned avail = reg.length - off;
                if (avail > HD2_BLOCK_31)
                    avail = HD2_BLOCK_31;
                unsigned radio_addr = reg.radio_base + (off / HD2_BLOCK_31) * 4;
                chunks.push_back({&reg, reg.file_base + off, radio_addr, avail, true});
            }
        }
        else
        {
            for (unsigned off = 0; off < reg.length; off += HD2_CHUNK_0F)
            {
                chunks.push_back({&reg, reg.file_base + off, reg.radio_base + off, HD2_CHUNK_0F, false});
            }
        }
    }
    return chunks;
}

// Build a write payload for a chunk from the file image, padding a short block
// tail with 0xFF (erased flash) so the frame is always full-length.
static auto hd2_chunk_payload(const HD2Chunk &c, const std::vector<uint8_t> &image) -> std::vector<uint8_t>
{
    unsigned full = c.is_block ? HD2_BLOCK_31 : HD2_CHUNK_0F;
    std::vector<uint8_t> buf(full, 0xFF);
    std::memcpy(buf.data(), image.data() + c.file_off, c.len);
    return buf;
}

// Write one chunk (full-length frame), dispatching on its protocol.
static auto hd2_write_chunk(int fd, const HD2Chunk &c, const std::vector<uint8_t> &image, int pct) -> void
{
    auto payload = hd2_chunk_payload(c, image);
    if (c.is_block)
        hd2_write_block_31(fd, c.radio_addr, pct, payload.data(), c.reg->name);
    else
        hd2_write_chunk_0f(fd, c.radio_addr, pct, payload.data(), c.reg->name);
}

// --- manifest helpers -------------------------------------------------------

static auto hd2_load_image_file(const std::string &file) -> std::vector<uint8_t>
{
    std::ifstream f(file, std::ios::binary);
    if (!f.is_open())
        throw std::runtime_error("Cannot open codeplug file: " + file);
    f.seekg(0, std::ios::end);
    auto len = static_cast<std::streamoff>(f.tellg());
    f.seekg(0, std::ios::beg);
    if (len != static_cast<std::streamoff>(HD2_MEMSZ))
    {
        std::stringstream ss;
        ss << "Unexpected codeplug size " << len << " bytes (expected 0x" << std::hex << HD2_MEMSZ << ")";
        throw std::runtime_error(ss.str());
    }
    std::vector<uint8_t> img(HD2_MEMSZ);
    f.read(reinterpret_cast<char *>(img.data()), HD2_MEMSZ);
    return img;
}

static auto rd_u32_le(const uint8_t *p) -> uint32_t
{
    return static_cast<uint32_t>(p[0]) | (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) | (static_cast<uint32_t>(p[3]) << 24);
}

static auto wr_u32_le(uint8_t *p, uint32_t v) -> void
{
    p[0] = v & 0xFF;
    p[1] = (v >> 8) & 0xFF;
    p[2] = (v >> 16) & 0xFF;
    p[3] = (v >> 24) & 0xFF;
}

// Parse a manifest out of a raw 4 KB manifest block (m points at the block
// start).  Returns the per-chunk CRC vector, or empty if the magic/version/
// count is invalid.
static auto hd2_parse_manifest(const uint8_t *m, size_t expected_chunks) -> std::vector<uint32_t>
{
    if (rd_u32_le(m) != HD2_MANIFEST_MAGIC)
        return {};
    if (rd_u32_le(m + 4) != HD2_MANIFEST_VERSION)
        return {};
    uint32_t count = rd_u32_le(m + 8);
    if (count != expected_chunks)
        return {};
    if (12 + count * 4 > HD2_MANIFEST_LEN)
        return {};
    std::vector<uint32_t> crcs(count);
    for (uint32_t i = 0; i < count; i++)
        crcs[i] = rd_u32_le(m + 12 + i * 4);
    return crcs;
}

// Confirm the channel slots overlapped by the reserved manifest block hold no
// real channel, per the codeplug's own presence bitmap.  We deliberately
// sacrifice the top channel slots to the manifest, but refuse to silently
// clobber a populated channel if a codeplug ever fills that far -- the caller
// gets a clear error instead of a corrupted upload.
static auto hd2_check_manifest_region_free(const std::vector<uint8_t> &image) -> void
{
    const uint8_t *bm = image.data() + HD2_BITMAP_OFF;
    unsigned block_end = HD2_MANIFEST_OFF + HD2_MANIFEST_LEN;
    for (unsigned s = 0;; s++)
    {
        unsigned slot = HD2_CHAN_BASE + s * HD2_CHAN_STRIDE;
        if (slot >= block_end)
            break;                                  // past the manifest block
        if (slot + HD2_CHAN_STRIDE <= HD2_MANIFEST_OFF)
            continue;                               // slot ends before the block
        if (s < 2)
            continue;                               // VFO-A/B presets, not bitmap-tracked
        unsigned ch = s - 2;                        // real channel index
        bool populated = (bm[ch >> 3] & (1u << (ch & 7))) == 0;  // CLEAR bit == populated
        if (populated)
        {
            std::stringstream ss;
            ss << "Codeplug channel #" << (ch + 1) << " falls in the reserved "
               << "delta-manifest block (file 0x" << std::hex << HD2_MANIFEST_OFF
               << "); refusing to overwrite it. This codeplug uses more channels "
               << "than the manifest reservation allows.";
            throw std::runtime_error(ss.str());
        }
    }
}

// Write a freshly computed manifest (magic/version/count + per-chunk CRCs)
// into the reserved block of image[].  The CRCs are computed over the
// full-length (0xFF-padded) chunk payloads so they match what gets written.
static auto hd2_build_manifest(std::vector<uint8_t> &image, const std::vector<HD2Chunk> &chunks) -> std::vector<uint32_t>
{
    std::vector<uint32_t> crcs(chunks.size());
    uint8_t *m = image.data() + HD2_MANIFEST_OFF;
    std::memset(m, 0xFF, HD2_MANIFEST_LEN);
    wr_u32_le(m, HD2_MANIFEST_MAGIC);
    wr_u32_le(m + 4, HD2_MANIFEST_VERSION);
    wr_u32_le(m + 8, static_cast<uint32_t>(chunks.size()));
    for (size_t i = 0; i < chunks.size(); i++)
    {
        auto payload = hd2_chunk_payload(chunks[i], image);
        crcs[i] = crc32(payload.data(), payload.size());
        wr_u32_le(m + 12 + i * 4, crcs[i]);
    }
    return crcs;
}
#endif // !_WIN32

auto AilunceRadio::WriteCodeplug(const std::string &file, const uint32_t &baud, const bool &delta) -> void
{
#ifdef _WIN32
    (void)file; (void)baud; (void)delta;
    throw std::runtime_error("Codeplug upload is not implemented on Windows in this build");
#else
    auto image = hd2_load_image_file(file);

    // Fail before touching the radio if the codeplug uses channels that the
    // reserved manifest block would clobber.
    hd2_check_manifest_region_free(image);

    auto fd = device.GetFD();
    hd2_serial_configure(fd, baud);

    auto chunks = hd2_enumerate_chunks();

    if (delta)
    {
        // Read only the radio's manifest block FIRST (GetVer session) to
        // recover its per-chunk CRCs -- the manifest already describes what is
        // on the radio, so there is no need to download the full image.  Then
        // open the write (SLC7000) session.
        std::vector<uint8_t> radio_manifest;
        hd2_read_manifest_block(fd, radio_manifest);

        auto old_crcs = hd2_parse_manifest(radio_manifest.data(), chunks.size());

        hd2_write_handshake(fd);

        if (old_crcs.empty())
        {
            // No valid manifest on the radio: full write + (re)build manifest.
            std::cerr << "No valid delta manifest on radio -- performing full write." << std::endl;
            hd2_build_manifest(image, chunks);
            for (size_t i = 0; i < chunks.size(); i++)
            {
                int pct = static_cast<int>(i * 100 / chunks.size());
                hd2_write_chunk(fd, chunks[i], image, pct);
                std::cerr << "#" << std::flush;
            }
            std::cerr << std::endl;
        }
        else
        {
            // Build the new manifest in the outgoing image, then write only the
            // chunks whose CRC differs.  The manifest chunk(s) are written last
            // (data-before-hash ordering): we defer any chunk overlapping the
            // reserved manifest block until the end.
            auto new_crcs = hd2_build_manifest(image, chunks);

            int written = 0;
            std::vector<size_t> deferred;
            for (size_t i = 0; i < chunks.size(); i++)
            {
                const auto &c = chunks[i];
                bool is_manifest = (c.file_off <= HD2_MANIFEST_OFF &&
                                    HD2_MANIFEST_OFF < c.file_off + (c.is_block ? HD2_BLOCK_31 : HD2_CHUNK_0F));
                if (is_manifest)
                {
                    deferred.push_back(i);
                    continue;
                }
                if (new_crcs[i] != old_crcs[i])
                {
                    int pct = static_cast<int>(i * 100 / chunks.size());
                    hd2_write_chunk(fd, c, image, pct);
                    written++;
                    std::cerr << "#" << std::flush;
                }
            }
            // Write the manifest-bearing chunk(s) last so the on-radio hash is
            // only updated after all data it describes has landed.
            for (auto i : deferred)
            {
                hd2_write_chunk(fd, chunks[i], image, 100);
                written++;
                std::cerr << "#" << std::flush;
            }
            std::cerr << std::endl;
            std::cerr << "Delta write: " << written << "/" << chunks.size() << " chunks changed." << std::endl;
        }
    }
    else
    {
        // Full write: open the write session, build/refresh the manifest in the
        // outgoing image so a later --delta run has a valid baseline, then write
        // every chunk.
        hd2_write_handshake(fd);
        hd2_build_manifest(image, chunks);
        for (size_t i = 0; i < chunks.size(); i++)
        {
            int pct = static_cast<int>(i * 100 / chunks.size());
            hd2_write_chunk(fd, chunks[i], image, pct);
            std::cerr << "#" << std::flush;
        }
        std::cerr << std::endl;
    }

    hd2_write_end(fd);
#endif
}

auto AilunceRadio::ReadCodeplug(const std::string &file, const uint32_t &baud) -> void
{
#ifdef _WIN32
    (void)file; (void)baud;
    throw std::runtime_error("Codeplug download is not implemented on Windows in this build");
#else
    auto fd = device.GetFD();
    hd2_serial_configure(fd, baud);

    std::vector<uint8_t> image;
    hd2_download_image(fd, image);

    std::ofstream out(file, std::ios::binary);
    if (!out.is_open())
        throw std::runtime_error("Cannot open output file: " + file);
    out.write(reinterpret_cast<const char *>(image.data()), image.size());
#endif
}