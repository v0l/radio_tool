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
#pragma once

namespace radio_tool::fw::cipher
{
    constexpr auto md9600_length = 1024;

    //md9600 fw encryption key
    // -KG5RKI
    const unsigned char md9600[md9600_length] = {
        0xa2, 0xfa, 0xbb, 0x4b, 0x90, 0x8f, 0x17, 0x20, 0x96, 0x36, 0x43, 0x84, 0xf7, 0xac, 0x4e, 0x55,
        0xea, 0xe5, 0xb4, 0x36, 0x55, 0xb9, 0x39, 0xe2, 0xd8, 0xda, 0x18, 0xc0, 0x0d, 0x09, 0x5d, 0xb8,
        0x0e, 0x89, 0x90, 0x46, 0x38, 0xd4, 0x93, 0xcc, 0x2f, 0x8e, 0xcd, 0x2d, 0x22, 0xb7, 0x89, 0x97,
        0x51, 0x24, 0x98, 0xa0, 0xcc, 0x30, 0x3e, 0x95, 0x7d, 0xaf, 0x4c, 0x0e, 0x68, 0x23, 0x89, 0xc6,
        0x32, 0x33, 0x56, 0xaa, 0xe0, 0x58, 0x92, 0x30, 0xe2, 0xda, 0xbc, 0xea, 0x50, 0xfb, 0x57, 0x5b,
        0x73, 0x71, 0x93, 0x09, 0x87, 0x1a, 0x29, 0xd3, 0xbf, 0xec, 0x87, 0x85, 0x8a, 0x2b, 0x2d, 0xaa,
        0x15, 0xde, 0x57, 0xa2, 0x11, 0x83, 0xdc, 0xf4, 0xb6, 0x02, 0x56, 0xe5, 0x08, 0xe0, 0x83, 0x49,
        0x59, 0xb5, 0xeb, 0x99, 0x0f, 0xe0, 0xc3, 0x46, 0xa7, 0x79, 0x12, 0x4d, 0xfa, 0x87, 0x12, 0x0c,
        0xbf, 0x73, 0xd9, 0x53, 0x52, 0xbd, 0x38, 0xbf, 0xb4, 0xee, 0xe4, 0x43, 0xd2, 0xce, 0xd3, 0x08,
        0x0a, 0xd6, 0xe9, 0x77, 0xeb, 0xe8, 0xd4, 0x94, 0x3c, 0x3e, 0x35, 0x8d, 0x40, 0xa1, 0x00, 0x92,
        0x39, 0xdb, 0x25, 0xe8, 0x2b, 0x6e, 0x70, 0x39, 0xe2, 0x86, 0xad, 0x2f, 0x36, 0x2d, 0x11, 0x41,
        0x8e, 0xbe, 0xd5, 0xcc, 0xa3, 0x9c, 0x24, 0x65, 0x87, 0x23, 0x37, 0x6e, 0xe5, 0xdf, 0xbf, 0xe7,
        0x8a, 0xfc, 0x83, 0x87, 0x24, 0xfe, 0x4a, 0x0b, 0x4a, 0xb3, 0xfb, 0xcf, 0xbd, 0x65, 0x03, 0x9b,
        0xee, 0x53, 0xf7, 0xbf, 0xc0, 0x63, 0x7a, 0x62, 0x8e, 0x11, 0x62, 0x17, 0x70, 0xab, 0x16, 0xb1,
        0xba, 0xc0, 0x3a, 0x59, 0xc6, 0xd6, 0x8f, 0xdd, 0xf4, 0x5b, 0x14, 0x4b, 0xee, 0xde, 0x72, 0xbf,
        0x31, 0x7f, 0x96, 0x79, 0xc9, 0xa4, 0xa0, 0x32, 0x5b, 0xee, 0xfc, 0xb0, 0x69, 0x6c, 0xce, 0x99,
        0xd2, 0x0e, 0x94, 0x85, 0x98, 0x5c, 0x07, 0x56, 0xe6, 0x67, 0x41, 0xcc, 0x52, 0x00, 0x25, 0x54,
        0x5f, 0x29, 0xfc, 0x21, 0x46, 0xc9, 0x5c, 0x7e, 0xf6, 0xa4, 0x4e, 0x63, 0x59, 0x89, 0xaf, 0x46,
        0xd9, 0xcd, 0xd7, 0x33, 0x23, 0xf9, 0x79, 0x1f, 0x2a, 0xc0, 0xca, 0x7a, 0x6f, 0x34, 0xe6, 0x03,
        0x81, 0x39, 0x6f, 0xe0, 0xbf, 0x39, 0x77, 0xee, 0x65, 0x19, 0xa0, 0x56, 0xc7, 0x6c, 0x81, 0x61,
        0xd7, 0xe7, 0x4c, 0x8d, 0xed, 0x15, 0xae, 0xe0, 0xc8, 0x4c, 0xf7, 0x7c, 0xd0, 0xe0, 0x7b, 0x74,
        0x9d, 0x96, 0x38, 0xde, 0xbd, 0x5c, 0xb9, 0x29, 0xb2, 0x37, 0x3a, 0xb1, 0x3b, 0x7c, 0x0c, 0x91,
        0xd5, 0x43, 0x3b, 0xb8, 0x80, 0x19, 0x6f, 0x40, 0xc6, 0xf5, 0x10, 0xfb, 0xfa, 0x6e, 0xad, 0x4e,
        0xbe, 0x2a, 0x9f, 0x42, 0xc7, 0x9a, 0xe9, 0xd8, 0xe5, 0xe4, 0x63, 0x9d, 0x3d, 0x21, 0x18, 0x7f,
        0xd9, 0xc9, 0xec, 0xdf, 0x64, 0x6b, 0x82, 0xe7, 0x2e, 0xa2, 0x5c, 0x1e, 0x77, 0x44, 0x44, 0x39,
        0xe9, 0xdc, 0xeb, 0x35, 0x66, 0x5b, 0xd1, 0xa2, 0x04, 0x0a, 0x64, 0x42, 0x56, 0xc3, 0x6c, 0xd2,
        0xee, 0x61, 0xa6, 0x28, 0x1f, 0x75, 0xaf, 0x7e, 0x08, 0x3b, 0x24, 0x0e, 0xcd, 0xcc, 0x08, 0xdf,
        0x28, 0x94, 0x66, 0xde, 0x21, 0x07, 0x37, 0x30, 0x19, 0x90, 0x85, 0xc7, 0x0d, 0xca, 0xd1, 0x33,
        0x19, 0xf3, 0xb3, 0xbb, 0x3b, 0x9e, 0xc0, 0xad, 0x5a, 0xa7, 0xb0, 0xf2, 0x87, 0x6c, 0xc1, 0xe5,
        0x82, 0x3a, 0x56, 0x66, 0x80, 0x06, 0xe4, 0x29, 0x2b, 0x5e, 0x0e, 0x54, 0xeb, 0x9f, 0x0f, 0x4a,
        0x64, 0x67, 0x59, 0xc1, 0x40, 0x4d, 0x7b, 0x1b, 0x2e, 0xd0, 0x48, 0xf3, 0x2a, 0x8e, 0x36, 0xf6,
        0x00, 0xb7, 0x04, 0xf4, 0x0b, 0xc0, 0xa0, 0x36, 0x43, 0x5c, 0x47, 0x13, 0x77, 0xa8, 0xee, 0xbe,
        0xd6, 0xa5, 0xe1, 0x62, 0xb4, 0xec, 0xaa, 0x71, 0x8b, 0x9d, 0x34, 0x39, 0x40, 0x99, 0x30, 0xb8,
        0xa8, 0xf1, 0xb8, 0xb1, 0x4b, 0x9e, 0x32, 0xff, 0x68, 0x72, 0x78, 0x2a, 0x39, 0x4e, 0x36, 0x38,
        0x77, 0x96, 0x93, 0xc5, 0x21, 0xe2, 0x13, 0x56, 0x7a, 0xf6, 0xbb, 0xeb, 0x51, 0xf5, 0x77, 0xd3,
        0x84, 0xd1, 0xba, 0xc4, 0xc7, 0x06, 0x64, 0x2b, 0xa2, 0x88, 0xe8, 0xc1, 0xb9, 0xf9, 0xae, 0x5f,
        0x50, 0x20, 0xb6, 0x13, 0x0e, 0x97, 0x7f, 0x73, 0x01, 0xc3, 0x27, 0x31, 0xe3, 0x09, 0xd3, 0xf0,
        0x9c, 0x3f, 0x51, 0x56, 0x07, 0x61, 0xfc, 0x63, 0xf9, 0x86, 0xe0, 0x01, 0x80, 0x12, 0x1f, 0xdc,
        0x68, 0x2c, 0x94, 0x73, 0x04, 0x73, 0xb5, 0x70, 0x2b, 0xec, 0xbe, 0x34, 0x80, 0x3f, 0x0c, 0xb7,
        0xf6, 0x24, 0xc6, 0x8f, 0x94, 0x18, 0xc3, 0x4e, 0x76, 0x54, 0xa8, 0x11, 0x15, 0xff, 0x51, 0x56,
        0xc8, 0xa3, 0x73, 0x0e, 0x8a, 0xde, 0x7f, 0xf4, 0xfd, 0x5a, 0xc9, 0x1c, 0xaf, 0xfe, 0xe9, 0xcf,
        0x9c, 0x66, 0x61, 0x96, 0xf5, 0x91, 0x81, 0x95, 0x20, 0xda, 0x88, 0x1a, 0x00, 0x2a, 0x0c, 0x76,
        0x76, 0x6b, 0x9c, 0x0c, 0x28, 0x40, 0xa3, 0xa7, 0x81, 0xf3, 0x8f, 0x11, 0xf9, 0xaf, 0x33, 0xe1,
        0x96, 0xef, 0x6a, 0x94, 0xb2, 0x36, 0xfe, 0xdf, 0x00, 0x01, 0xc8, 0x44, 0xca, 0xf9, 0x18, 0xe4,
        0x7c, 0x6e, 0x57, 0x94, 0x66, 0x01, 0xea, 0x32, 0xbe, 0xa0, 0x5a, 0x3a, 0xe4, 0xb8, 0xb2, 0x94,
        0xea, 0xa5, 0x29, 0xb0, 0x54, 0x6e, 0x01, 0xd5, 0x1c, 0xaf, 0xaf, 0xb6, 0xfa, 0xd6, 0x3c, 0x47,
        0xe2, 0x92, 0xeb, 0xce, 0xcd, 0x89, 0x1c, 0x3d, 0xbc, 0x4a, 0x70, 0xbf, 0xfa, 0x82, 0x2e, 0x91,
        0xa2, 0x72, 0xe6, 0x13, 0x62, 0xa0, 0x54, 0x1f, 0x7e, 0xcd, 0x86, 0x99, 0x18, 0x28, 0x41, 0x47,
        0xae, 0xc1, 0xa2, 0xe3, 0xe4, 0x40, 0x01, 0x6f, 0x84, 0xd7, 0x1a, 0xc9, 0xc3, 0x75, 0x6f, 0x7f,
        0xc6, 0x3d, 0xe8, 0xe4, 0x64, 0x36, 0xbd, 0x64, 0x2e, 0x44, 0x95, 0x14, 0xac, 0x57, 0xf0, 0x8d,
        0xea, 0xe2, 0xc2, 0xfb, 0x33, 0x8f, 0x60, 0x71, 0x1d, 0x31, 0xa0, 0x80, 0xc6, 0xf9, 0x3c, 0x07,
        0x5c, 0xee, 0x78, 0x4c, 0xe3, 0x97, 0x05, 0x4c, 0x32, 0xfa, 0x24, 0x50, 0x3f, 0xcb, 0x0f, 0xc1,
        0x9d, 0xdd, 0x94, 0x3d, 0x43, 0xdc, 0x03, 0xea, 0x8f, 0x3e, 0x4a, 0x0b, 0x8b, 0x77, 0x5f, 0xd1,
        0x6e, 0x6c, 0xde, 0x73, 0x66, 0x2b, 0xf4, 0x81, 0x94, 0xd9, 0x7b, 0x75, 0x58, 0xeb, 0x66, 0x8b,
        0xd0, 0x9a, 0x60, 0xd2, 0x9b, 0x90, 0xb0, 0x83, 0xe3, 0xe8, 0x60, 0x92, 0x9a, 0x55, 0x9e, 0x84,
        0x03, 0xa1, 0x62, 0x80, 0x75, 0x5a, 0x51, 0xa8, 0x5c, 0xc8, 0xe2, 0xaa, 0x80, 0x21, 0xbf, 0x91,
        0x8a, 0x00, 0x6e, 0xe2, 0xc4, 0x14, 0x30, 0xe4, 0x20, 0x15, 0x29, 0x3f, 0x7c, 0xfd, 0xc2, 0xc8,
        0x24, 0x74, 0x4c, 0x9c, 0x98, 0x8c, 0xe6, 0x6c, 0x90, 0xae, 0xa0, 0x17, 0x3e, 0xd5, 0xe0, 0x7e,
        0xd3, 0xf9, 0x05, 0x94, 0x44, 0xcf, 0x4b, 0xb4, 0x4e, 0xaf, 0xee, 0x38, 0xb8, 0xd5, 0x93, 0x47,
        0xd8, 0xcd, 0xe3, 0xee, 0x58, 0x29, 0x79, 0x72, 0x3a, 0x75, 0xfe, 0xe5, 0x1a, 0x6d, 0x92, 0xf8,
        0xb3, 0x6d, 0x6e, 0x10, 0xa5, 0x28, 0xc8, 0x9c, 0x76, 0x9d, 0xf7, 0xa5, 0xd6, 0x47, 0xd8, 0xa6,
        0x27, 0x94, 0x70, 0x9f, 0x3c, 0x99, 0xd3, 0x65, 0x61, 0x04, 0x44, 0x3c, 0x9c, 0x52, 0x9d, 0xa7,
        0x33, 0x42, 0xf2, 0x7f, 0x6e, 0x89, 0x71, 0x43, 0x9e, 0xc7, 0x8c, 0xaf, 0x5e, 0xba, 0x5b, 0x90,
        0x19, 0xb1, 0x3b, 0xd6, 0xcd, 0x44, 0xbc, 0xeb, 0x0e, 0x43, 0xba, 0x43, 0x4d, 0xec, 0xc9, 0x35
    };
} // namespace radio_tool::fw::cipher