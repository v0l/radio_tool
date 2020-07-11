cmake_minimum_required(VERSION 3.0.0)
project(tyt_tool VERSION 0.1.0)

include(CTest)
enable_testing()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(ALL_SRC 
    src/tyt_tool.cpp
    src/dfu.cpp
    src/tyt_dfu.cpp
)
add_executable(tyt_tool ${ALL_SRC})
target_include_directories(tyt_tool PUBLIC include)
target_link_libraries(tyt_tool usb-1.0)

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
include(CPack)
