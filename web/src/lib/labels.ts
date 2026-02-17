import type {
  Attribute,
  Campus,
  HybridVariant,
  InstructionalMethod,
  OnlineVariant,
  PartOfTerm,
} from "$lib/bindings";

/** Display label styles for different UI contexts */
export type DisplayContext = "filter" | "detail" | "tooltip";

const ONLINE_VARIANT_LABELS: Record<OnlineVariant, Record<DisplayContext, string>> = {
  Async: {
    filter: "Online Async",
    detail: "Online Async",
    tooltip: "Fully online, no scheduled meeting times",
  },
  Sync: {
    filter: "Online Sync",
    detail: "Online Sync",
    tooltip: "Fully online, all meetings at scheduled times",
  },
  Mixed: {
    filter: "Online Mix",
    detail: "Online (Some Live)",
    tooltip: "Fully online with some synchronous meetings (discontinued)",
  },
};

const HYBRID_VARIANT_LABELS: Record<HybridVariant, Record<DisplayContext, string>> = {
  Half: {
    filter: "Hybrid",
    detail: "Hybrid (Half)",
    tooltip: "1 of 2 meeting days in person, rest online",
  },
  OneThird: {
    filter: "Hybrid",
    detail: "Hybrid (Mostly Online)",
    tooltip: "1 of 3 meeting days in person, rest online",
  },
  TwoThirds: {
    filter: "Hybrid",
    detail: "Hybrid (Mostly In Person)",
    tooltip: "2 of 3 meeting days in person, rest online",
  },
};

export function getInstructionalMethodLabel(
  method: InstructionalMethod,
  context: DisplayContext
): string {
  switch (method.type) {
    case "InPerson":
      return context === "tooltip" ? "Traditional in-person course" : "In Person";
    case "Online":
      return ONLINE_VARIANT_LABELS[method.variant][context];
    case "Hybrid":
      return HYBRID_VARIANT_LABELS[method.variant][context];
    case "Independent":
      return context === "tooltip"
        ? "Meetings arranged between faculty and student"
        : context === "detail"
          ? "Independent Study"
          : "Independent";
  }
}

const CAMPUS_LABELS: Record<Exclude<Campus["type"], "Unknown">, Record<DisplayContext, string>> = {
  Main: { filter: "Main", detail: "Main Campus", tooltip: "UTSA Main Campus, San Antonio" },
  Downtown: { filter: "Downtown", detail: "Downtown", tooltip: "UTSA Downtown Campus" },
  Southwest: { filter: "Southwest", detail: "Southwest", tooltip: "UTSA Southwest Campus" },
  Laredo: { filter: "Laredo", detail: "Laredo", tooltip: "Laredo Education Center" },
  Internet: {
    filter: "Internet",
    detail: "Internet",
    tooltip: "Online courses for campus students",
  },
  OnlinePrograms: {
    filter: "Online Programs",
    detail: "Online Programs",
    tooltip: "Restricted to online degree program students",
  },
};

export function getCampusLabel(campus: Campus, context: DisplayContext): string {
  if (campus.type === "Unknown") {
    return context === "filter" ? campus.code : campus.description || campus.code;
  }
  return CAMPUS_LABELS[campus.type][context];
}

const ATTRIBUTE_LABELS: Record<
  Exclude<Attribute["type"], "Unknown">,
  Record<DisplayContext, string>
> = {
  // Core Curriculum
  CoreCommunication: {
    filter: "Communication",
    detail: "Core: Communication",
    tooltip: "Communication (010)",
  },
  CoreMathematics: {
    filter: "Mathematics",
    detail: "Core: Mathematics",
    tooltip: "Mathematics (020)",
  },
  CoreLifePhysicalSciences: {
    filter: "Life/Physical Sci",
    detail: "Core: Life & Physical Sciences",
    tooltip: "Life and Physical Sciences (030)",
  },
  CoreLanguagePhilosophy: {
    filter: "Language/Phil",
    detail: "Core: Language, Philosophy & Culture",
    tooltip: "Language, Philosophy & Culture (040)",
  },
  CoreCreativeArts: {
    filter: "Creative Arts",
    detail: "Core: Creative Arts",
    tooltip: "Creative Arts (050)",
  },
  CoreAmericanHistory: {
    filter: "US History",
    detail: "Core: American History",
    tooltip: "American History (060)",
  },
  CoreGovernment: {
    filter: "Government",
    detail: "Core: Government/Political Science",
    tooltip: "Government/Political Science (070)",
  },
  CoreSocialBehavioral: {
    filter: "Social/Behavioral",
    detail: "Core: Social & Behavioral Sciences",
    tooltip: "Social and Behavioral Sciences (080)",
  },
  CoreComponentArea: {
    filter: "Component Area",
    detail: "Core: Component Area Option",
    tooltip: "Component Area Option (090)",
  },

  // Course Level
  Developmental: {
    filter: "Developmental",
    detail: "Developmental",
    tooltip: "Developmental course",
  },
  LowerDivision: {
    filter: "Lower Div",
    detail: "Lower Division",
    tooltip: "Lower division (1000-2000 level)",
  },
  UpperDivision: {
    filter: "Upper Div",
    detail: "Upper Division",
    tooltip: "Upper division (3000-4000 level)",
  },
  Graduate: { filter: "Graduate", detail: "Graduate", tooltip: "Graduate level course" },

  // Special Designations
  Honors: { filter: "Honors", detail: "Honors", tooltip: "Honors section" },
  LowCostTextbooks: {
    filter: "Low Cost",
    detail: "Low Cost Textbooks",
    tooltip: "Low cost textbook section (under $50)",
  },
  FreeTextbooks: {
    filter: "Free Text",
    detail: "Free Textbooks",
    tooltip: "No cost textbook section",
  },
  Leadership: { filter: "Leadership", detail: "Leadership", tooltip: "Leadership designation" },
  ServiceLearning: {
    filter: "Service",
    detail: "Service Learning",
    tooltip: "Service learning component",
  },
  FinishAtUT: { filter: "Finish@UT", detail: "Finish in Four", tooltip: "Finish at UT course" },
  UndergraduateResearch: {
    filter: "UG Research",
    detail: "Undergraduate Research",
    tooltip: "Undergraduate research component",
  },
};

export function getAttributeLabel(attr: Attribute, context: DisplayContext): string {
  if (attr.type === "Unknown") {
    return context === "filter" ? attr.code : attr.description || attr.code;
  }
  return ATTRIBUTE_LABELS[attr.type][context];
}

const PART_OF_TERM_LABELS: Record<
  Exclude<PartOfTerm["type"], "Unknown">,
  Record<DisplayContext, string>
> = {
  FullTerm: { filter: "Full Term", detail: "Full Term", tooltip: "Full semester" },
  FirstHalf: { filter: "First Half", detail: "First Half", tooltip: "First half of semester" },
  SecondHalf: { filter: "Second Half", detail: "Second Half", tooltip: "Second half of semester" },
};

export function getPartOfTermLabel(pot: PartOfTerm, context: DisplayContext): string {
  if (pot.type === "Unknown") {
    return context === "filter" ? pot.code : pot.description || pot.code;
  }
  return PART_OF_TERM_LABELS[pot.type][context];
}

// --- Filter value string → display label bridge functions ---

function stripRawPrefix(value: string): string {
  return value.startsWith("raw:") ? value.slice(4) : value;
}

const IM_FILTER_LABELS: Record<string, string> = {
  InPerson: "In Person",
  "Online.Async": "Online Async",
  "Online.Sync": "Online Sync",
  "Online.Mixed": "Online Mix",
  "Hybrid.Half": "Hybrid Half",
  "Hybrid.OneThird": "Hybrid One Third",
  "Hybrid.TwoThirds": "Hybrid Two Thirds",
  Independent: "Independent",
};

export function getInstructionalMethodFilterLabel(filterValue: string): string {
  return IM_FILTER_LABELS[filterValue] ?? stripRawPrefix(filterValue);
}

const CAMPUS_FILTER_LABELS: Record<string, string> = Object.fromEntries(
  Object.entries(CAMPUS_LABELS).map(([key, labels]) => [key, labels.filter])
);

export function getCampusFilterLabel(filterValue: string): string {
  return CAMPUS_FILTER_LABELS[filterValue] ?? stripRawPrefix(filterValue);
}

const ATTRIBUTE_FILTER_LABELS: Record<string, string> = Object.fromEntries(
  Object.entries(ATTRIBUTE_LABELS).map(([key, labels]) => [key, labels.filter])
);

export function getAttributeFilterLabel(filterValue: string): string {
  return ATTRIBUTE_FILTER_LABELS[filterValue] ?? stripRawPrefix(filterValue);
}

const POT_FILTER_LABELS: Record<string, string> = Object.fromEntries(
  Object.entries(PART_OF_TERM_LABELS).map(([key, labels]) => [key, labels.filter])
);

export function getPartOfTermFilterLabel(filterValue: string): string {
  return POT_FILTER_LABELS[filterValue] ?? stripRawPrefix(filterValue);
}

export function getFilterLabel(
  category: "instructionalMethod" | "campus" | "attribute" | "partOfTerm",
  filterValue: string
): string {
  switch (category) {
    case "instructionalMethod":
      return getInstructionalMethodFilterLabel(filterValue);
    case "campus":
      return getCampusFilterLabel(filterValue);
    case "attribute":
      return getAttributeFilterLabel(filterValue);
    case "partOfTerm":
      return getPartOfTermFilterLabel(filterValue);
  }
}

// --- Shared filter UI configuration ---

/** Attribute filter value groupings for UI categorization */
export const ATTRIBUTE_GROUPS = {
  core: new Set([
    "CoreCommunication",
    "CoreMathematics",
    "CoreLifePhysicalSciences",
    "CoreLanguagePhilosophy",
    "CoreCreativeArts",
    "CoreAmericanHistory",
    "CoreGovernment",
    "CoreSocialBehavioral",
    "CoreComponentArea",
  ]),
  level: new Set(["Developmental", "LowerDivision", "UpperDivision", "Graduate"]),
} as const;

/** Campus filter value groupings for availability UI */
export const CAMPUS_GROUPS = {
  /** Campuses accessible to traditional campus students */
  campusStudents: ["Main", "Downtown", "Southwest", "Laredo", "Internet"] as string[],
  /** Campuses restricted to online degree program students */
  onlinePrograms: ["OnlinePrograms"] as string[],
};

/** Button configuration for instructional method filter UI */
export interface FormatButtonConfig {
  label: string;
  codes: string[];
  variants?: { code: string; label: string }[];
}

export const FORMAT_BUTTONS: FormatButtonConfig[] = [
  { label: "In Person", codes: ["InPerson"] },
  {
    label: "Online",
    codes: ["Online.Async", "Online.Sync", "Online.Mixed"],
    variants: [
      { code: "Online.Async", label: "Async" },
      { code: "Online.Sync", label: "Sync" },
    ],
  },
  {
    label: "Hybrid",
    codes: ["Hybrid.Half", "Hybrid.OneThird", "Hybrid.TwoThirds"],
    variants: [
      { code: "Hybrid.Half", label: "Half" },
      { code: "Hybrid.OneThird", label: "One Third" },
      { code: "Hybrid.TwoThirds", label: "Two Thirds" },
    ],
  },
  { label: "Independent", codes: ["Independent"] },
];
