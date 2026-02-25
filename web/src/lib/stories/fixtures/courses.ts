/**
 * Mock course data for Storybook stories.
 * These fixtures provide realistic data for testing course-related components.
 */
import type { CourseResponse } from "$lib/bindings/CourseResponse";

/**
 * A course with available seats
 */
export const courseWithSeats: CourseResponse = {
  crn: "12345",
  subject: "CS",
  courseNumber: "3443",
  title: "Application Programming",
  termCode: "202510",
  termSlug: "Spring-2025",
  sequenceNumber: "001",
  instructionalMethod: { type: "InPerson" },
  instructionalMethodCode: "P",
  campus: { type: "Main" },
  enrollment: {
    current: 22,
    max: 35,
    waitCount: 0,
    waitCapacity: 10,
  },
  creditHours: {
    type: "fixed",
    hours: 3,
  },
  crossList: null,
  sectionLink: null,
  partOfTerm: { type: "FullTerm" },
  meetingTimes: [
    {
      timeRange: { start: "09:30", end: "10:45" },
      dateRange: { start: "2025-01-13", end: "2025-05-08" },
      days: ["tuesday", "thursday"],
      location: {
        building: "MH",
        buildingDescription: "Main Building",
        room: "2.206",
        campus: "Main",
      },
      meetingType: "Lecture",
      meetingScheduleType: "LEC",
    },
  ],
  attributes: [],
  isAsyncOnline: false,
  primaryLocation: "MH 2.206",
  hasPhysicalLocation: true,
  primaryInstructorId: 1001,
  instructors: [
    {
      instructorId: 1001,
      bannerId: "abc123",
      displayName: "John Smith",
      firstName: "John",
      lastName: "Smith",
      email: "john.smith@university.edu",
      slug: "john-smith-abc",
      isPrimary: true,
      rmp: {
        avgRating: 4.2,
        numRatings: 45,
        legacyId: 123456,
      },
      bluebook: null,
      rating: {
        score: 4.2,
        rankScore: 3.8,
        ciLower: 3.5,
        ciUpper: 4.9,
        confidence: 0.7,
        source: "rmp",
        totalResponses: 45,
      },
    },
  ],
};

/**
 * A course that is completely full
 */
export const fullCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12346",
  subject: "CS",
  courseNumber: "1083",
  title: "Introduction to Computer Science",
  enrollment: {
    current: 40,
    max: 40,
    waitCount: 5,
    waitCapacity: 10,
  },
  instructors: [
    {
      instructorId: 1002,
      bannerId: "def456",
      displayName: "Jane Doe",
      firstName: "Jane",
      lastName: "Doe",
      email: "jane.doe@university.edu",
      slug: "jane-doe-def",
      isPrimary: true,
      rmp: null,
      bluebook: null,
      rating: null,
    },
  ],
  primaryInstructorId: 1002,
};

/**
 * An online course
 */
export const onlineCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12347",
  subject: "ENG",
  courseNumber: "1013",
  title: "Freshman Composition I",
  instructionalMethod: { type: "Online", variant: "Async" },
  instructionalMethodCode: "INET",
  campus: { type: "Internet" },
  isAsyncOnline: true,
  primaryLocation: "Online",
  hasPhysicalLocation: false,
  enrollment: {
    current: 18,
    max: 25,
    waitCount: 0,
    waitCapacity: 5,
  },
  meetingTimes: [
    {
      timeRange: null,
      dateRange: { start: "2025-01-13", end: "2025-05-08" },
      days: [],
      location: null,
      meetingType: "Web",
      meetingScheduleType: "WEB",
    },
  ],
  instructors: [
    {
      instructorId: 1003,
      bannerId: "ghi789",
      displayName: "Robert Johnson",
      firstName: "Robert",
      lastName: "Johnson",
      email: "robert.johnson@university.edu",
      slug: "robert-johnson-ghi",
      isPrimary: true,
      rmp: {
        avgRating: 3.8,
        numRatings: 120,
        legacyId: 234567,
      },
      bluebook: null,
      rating: {
        score: 3.8,
        rankScore: 3.4,
        ciLower: 3.2,
        ciUpper: 4.4,
        confidence: 0.8,
        source: "rmp",
        totalResponses: 120,
      },
    },
  ],
  primaryInstructorId: 1003,
};

/**
 * A course with limited seats (less than 5)
 */
export const lowSeatsCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12348",
  subject: "MAT",
  courseNumber: "2214",
  title: "Calculus II",
  enrollment: {
    current: 28,
    max: 30,
    waitCount: 2,
    waitCapacity: 5,
  },
};

/**
 * A course with Staff instructor
 */
export const staffInstructorCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12349",
  subject: "PHY",
  courseNumber: "1923",
  title: "Physics for Scientists I",
  instructors: [],
  primaryInstructorId: null,
};

/**
 * A course with multiple meeting times
 */
export const multiMeetingCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12350",
  subject: "CHE",
  courseNumber: "1103",
  title: "General Chemistry I",
  meetingTimes: [
    {
      timeRange: { start: "10:00", end: "10:50" },
      dateRange: { start: "2025-01-13", end: "2025-05-08" },
      days: ["monday", "wednesday", "friday"],
      location: {
        building: "SC",
        buildingDescription: "Science Building",
        room: "1.102",
        campus: "Main",
      },
      meetingType: "Lecture",
      meetingScheduleType: "LEC",
    },
    {
      timeRange: { start: "14:00", end: "16:50" },
      dateRange: { start: "2025-01-13", end: "2025-05-08" },
      days: ["thursday"],
      location: {
        building: "SC",
        buildingDescription: "Science Building",
        room: "2.301",
        campus: "Main",
      },
      meetingType: "Laboratory",
      meetingScheduleType: "LAB",
    },
  ],
  creditHours: {
    type: "fixed",
    hours: 4,
  },
};

/**
 * A course that is overenrolled (more students than capacity)
 */
export const overenrolledCourse: CourseResponse = {
  ...courseWithSeats,
  crn: "12351",
  subject: "CS",
  courseNumber: "2123",
  title: "Data Structures",
  enrollment: {
    current: 42,
    max: 40,
    waitCount: 0,
    waitCapacity: 10,
  },
};

/**
 * Array of all mock courses for list/table stories
 */
export const mockCourses: CourseResponse[] = [
  courseWithSeats,
  fullCourse,
  onlineCourse,
  lowSeatsCourse,
  staffInstructorCourse,
  multiMeetingCourse,
  overenrolledCourse,
];
