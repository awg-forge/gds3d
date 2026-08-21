export type ThemeColors = {
  bg: string;
  bgSecondary: string;
  bgTertiary: string;
  primary: string;
  primarySolid: string;
  primarySolidHover: string;
  secondary: string;
  textPrimary: string;
  textSecondary: string;
  border: string;
};

export type ThemeDefinition = {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  light: ThemeColors;
  dark: ThemeColors;
  lightAcrylic: ThemeColors;
  darkAcrylic: ThemeColors;
};

const lightColors = {
  bg: "#f3f3f3",
  bgSecondary: "#f9f9f9",
  bgTertiary: "#ffffff",
  primary: "#0067c0",
  primarySolid: "#0b83d5",
  primarySolidHover: "#0870ba",
  secondary: "#454b52",
  textPrimary: "#1b1b1b",
  textSecondary: "#5f5f5f",
  border: "#d6d6d6",
};
const darkColors = {
  bg: "#202020",
  bgSecondary: "#292929",
  bgTertiary: "#333333",
  primary: "#60cdff",
  primarySolid: "#0b83d5",
  primarySolidHover: "#2794df",
  secondary: "#8f9baa",
  textPrimary: "#f5f5f5",
  textSecondary: "#c4c4c4",
  border: "#454545",
};

const defaultTheme: ThemeDefinition = {
  id: "default",
  name: "Default",
  description: "Native platform colors",
  author: "AWGForge",
  version: "1.0.0",
  light: lightColors,
  dark: darkColors,
  lightAcrylic: lightColors,
  darkAcrylic: darkColors,
};

export default defaultTheme;
