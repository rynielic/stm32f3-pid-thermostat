#include "unity.h"
#include "pid.h"

#include <stdbool.h>

void setUp(void) {}
void tearDown(void) {}

void pid_new_sets_zeros_correctly(void)
{
    struct Pid pid = pid_new(1.0, 2.0, 3.0, 40.0);

    TEST_ASSERT_FLOAT_WITHIN(0.01f, 0.0, pid.previous_error);
    TEST_ASSERT_FLOAT_WITHIN(0.01f, 0.0, pid.integral);
}

void regression_test_for_integral(void)
{
    struct Pid pid = pid_new(1.0, 2.0, 3.0, 40.0);
    float integral0 = pid.integral;

    pid_output(&pid, 36.0, 1.0);
    float integral1 = pid.integral;

    pid_output(&pid, 38.0, 1.0);
    float integral2 = pid.integral;

    TEST_ASSERT_TRUE(integral0 < integral1);
    TEST_ASSERT_TRUE(integral1 < integral2);
}

void pass_only_kp(void)
{
    struct Pid pid = pid_new(1.0, 0.0, 0.0, 40.0);

    TEST_ASSERT_FLOAT_WITHIN(0.01f, 2.0 , pid_output(&pid, 38, 1.0));
}

void bang_bang_threshold_clamps_correctly(void)
{
    struct Pid pid = pid_new(1.0, 2.0, 3.0, 60.0);

    TEST_ASSERT_FLOAT_WITHIN(0.01f, 100.0 , pid_output(&pid, 50, 1.0));
    TEST_ASSERT_TRUE(pid_output(&pid, 58, 1.0) < 100.0);
}

void dt_does_not_crash(void)
{
    struct Pid pid = pid_new(1.0, 2.0, 3.0, 60.0);

    TEST_ASSERT_FLOAT_WITHIN(0.01f, 0.0 , pid_output(&pid, 50, 0.0));
}

int main(void)
{
    UNITY_BEGIN();
    RUN_TEST(pid_new_sets_zeros_correctly);
    RUN_TEST(regression_test_for_integral);
    RUN_TEST(pass_only_kp);
    RUN_TEST(bang_bang_threshold_clamps_correctly);
    RUN_TEST(dt_does_not_crash);
    return UNITY_END();
}
