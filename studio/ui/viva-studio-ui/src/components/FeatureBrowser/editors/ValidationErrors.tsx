import type { ValueError } from "../../../xml_model/values";

interface ValidationErrorsProps {
  errors: ValueError[];
}

// Shared error list so editors don't duplicate rendering logic.
export function ValidationErrors({ errors }: ValidationErrorsProps) {
  if (!errors || errors.length === 0) {
    return null;
  }

  return (
    <ul className="validation-errors">
      {errors.map((error, index) => (
        <li key={`${error.message}-${index}`}>{error.message}</li>
      ))}
    </ul>
  );
}
